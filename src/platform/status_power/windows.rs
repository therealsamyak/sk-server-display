use std::{cell::RefCell, collections::HashMap, rc::Rc};

use anyhow::{Context as _, Result, ensure};
use gpui::{BackgroundExecutor, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            Power::{
                GetSystemPowerStatus, HPOWERNOTIFY, POWER_REQUEST_TYPE, PowerClearRequest,
                PowerCreateRequest, PowerRequestDisplayRequired, PowerRequestSystemRequired,
                PowerSetRequest, RegisterPowerSettingNotification, SYSTEM_POWER_STATUS,
                UnregisterPowerSettingNotification,
            },
            SystemServices::{GUID_ACDC_POWER_SOURCE, POWER_REQUEST_CONTEXT_VERSION},
            Threading::{POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0},
        },
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                DEVICE_NOTIFY_WINDOW_HANDLE, PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND,
                PBT_POWERSETTINGCHANGE, WM_NCDESTROY, WM_POWERBROADCAST,
            },
        },
    },
    core::PWSTR,
};

use super::{IDLE_INHIBITION_REASON, format_windows_battery, update_idle_inhibition};

type WindowInhibitor = Rc<RefCell<Option<IdlePowerRequests>>>;

thread_local! {
    // Keep callback state on the window thread and route by the subclass ID. A callback
    // arriving after teardown cannot dereference stale data.
    static WINDOW_INHIBITORS: RefCell<HashMap<usize, WindowInhibitor>> = RefCell::default();
}

/// Owns the Windows idle requests and AC power notification.
pub(super) struct DisplayPowerMonitor {
    hwnd: HWND,
    inhibitor: WindowInhibitor,
    notification: Option<HPOWERNOTIFY>,
}

impl DisplayPowerMonitor {
    /// Watches AC power through the status window.
    pub(super) fn start(window: &Window, _: &BackgroundExecutor) -> Result<Self> {
        let native_handle = HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("could not read status window handle: {error}"))?;
        let RawWindowHandle::Win32(handle) = native_handle.as_raw() else {
            anyhow::bail!("status window has no Windows handle");
        };
        let hwnd = HWND(std::ptr::without_provenance_mut(
            handle.hwnd.get().cast_unsigned(),
        ));
        let inhibitor = Rc::new(RefCell::new(None));
        let id = Rc::as_ptr(&inhibitor).addr();

        // SAFETY: GPUI calls this on the thread that owns the live status window.
        let installed = unsafe { SetWindowSubclass(hwnd, Some(on_power_message), id, 0) };
        ensure!(
            installed.as_bool(),
            "could not observe status window power changes"
        );
        WINDOW_INHIBITORS.with_borrow_mut(|windows| windows.insert(id, Rc::clone(&inhibitor)));

        let mut monitor = Self {
            hwnd,
            inhibitor,
            notification: None,
        };
        // SAFETY: the HWND belongs to this thread and the GUID is valid for the call.
        let notification = unsafe {
            RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &GUID_ACDC_POWER_SOURCE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        }
        .context("could not register for AC power changes")?;
        monitor.notification = Some(notification);
        refresh_idle_inhibition(&monitor.inhibitor);
        Ok(monitor)
    }
}

impl Drop for DisplayPowerMonitor {
    fn drop(&mut self) {
        if remove_window_inhibitor(Rc::as_ptr(&self.inhibitor).addr()) {
            // SAFETY: the route is present only for the subclass on this thread's live window.
            if !unsafe {
                RemoveWindowSubclass(
                    self.hwnd,
                    Some(on_power_message),
                    Rc::as_ptr(&self.inhibitor).addr(),
                )
            }
            .as_bool()
            {
                log::warn!("could not remove status power window subclass");
            }
        }
        if let Some(notification) = self.notification {
            // SAFETY: this monitor owns the registration and unregisters it once.
            if let Err(error) = unsafe { UnregisterPowerSettingNotification(notification) } {
                log::warn!("could not unregister AC power changes: {error}");
            }
        }
    }
}

fn remove_window_inhibitor(id: usize) -> bool {
    if let Some(inhibitor) = WINDOW_INHIBITORS.with_borrow_mut(|windows| windows.remove(&id)) {
        inhibitor.replace(None);
        true
    } else {
        false
    }
}

unsafe extern "system" fn on_power_message(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    match message {
        WM_POWERBROADCAST => {
            let inhibitor = WINDOW_INHIBITORS.with_borrow(|windows| windows.get(&id).cloned());
            if let Some(inhibitor) = inhibitor {
                match u32::try_from(wparam.0) {
                    Ok(PBT_POWERSETTINGCHANGE) => refresh_idle_inhibition(&inhibitor),
                    Ok(PBT_APMSUSPEND) => {
                        inhibitor.replace(None);
                    }
                    Ok(PBT_APMRESUMEAUTOMATIC) => {
                        // Windows cancels active power requests on user-initiated sleep.
                        inhibitor.replace(None);
                        refresh_idle_inhibition(&inhibitor);
                    }
                    _ => {}
                }
            }
        }
        WM_NCDESTROY => {
            remove_window_inhibitor(id);
        }
        _ => {}
    }
    // SAFETY: forward the message and unchanged arguments from the subclass callback.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn read_system_power_status() -> windows::core::Result<SYSTEM_POWER_STATUS> {
    let mut power = SYSTEM_POWER_STATUS::default();
    // SAFETY: `power` is a writable, correctly typed status buffer.
    unsafe { GetSystemPowerStatus(&mut power) }?;
    Ok(power)
}

/// Formats Windows battery percentage and power state for the dashboard.
pub(super) fn windows_battery_text() -> Option<String> {
    match read_system_power_status() {
        Ok(power) => format_windows_battery(
            power.ACLineStatus,
            power.BatteryFlag,
            power.BatteryLifePercent,
        ),
        Err(error) => {
            log::debug!("could not read local battery status: {error}");
            None
        }
    }
}

fn refresh_idle_inhibition(inhibitor: &WindowInhibitor) {
    let on_ac = read_system_power_status().is_ok_and(|power| power.ACLineStatus == 1);
    update_idle_inhibition(
        &mut inhibitor.borrow_mut(),
        on_ac,
        IdlePowerRequests::acquire,
    );
}

const IDLE_REQUESTS: [POWER_REQUEST_TYPE; 2] =
    [PowerRequestDisplayRequired, PowerRequestSystemRequired];

struct IdlePowerRequests {
    handle: HANDLE,
    active: usize,
    _reason: Vec<u16>,
}

impl IdlePowerRequests {
    fn acquire() -> Result<Self> {
        let mut reason: Vec<u16> = IDLE_INHIBITION_REASON.encode_utf16().chain([0]).collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason.as_mut_ptr()),
            },
        };
        // SAFETY: the context uses the simple-string variant. The UTF-16 buffer remains
        // allocated until after the handle closes.
        let handle =
            unsafe { PowerCreateRequest(&context) }.context("could not create power request")?;
        let mut requests = Self {
            handle,
            active: 0,
            _reason: reason,
        };
        for request in IDLE_REQUESTS {
            // SAFETY: `requests` owns the live power request handle.
            unsafe { PowerSetRequest(handle, request) }
                .context("could not set idle power request")?;
            requests.active += 1;
        }
        Ok(requests)
    }
}

impl Drop for IdlePowerRequests {
    fn drop(&mut self) {
        for request in IDLE_REQUESTS.into_iter().take(self.active) {
            // SAFETY: every cleared request was set on this live handle.
            if let Err(error) = unsafe { PowerClearRequest(self.handle, request) } {
                log::warn!("could not clear idle power request: {error}");
            }
        }
        // SAFETY: this guard owns the handle and closes it once.
        if let Err(error) = unsafe { CloseHandle(self.handle) } {
            log::warn!("could not close idle power request: {error}");
        }
    }
}
