use std::{cell::RefCell, collections::HashMap, rc::Rc};

use anyhow::{Context as _, Result, ensure};
use gpui::{BackgroundExecutor, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            Power::{
                BATTERY_UNKNOWN_RATE, BATTERY_UNKNOWN_TIME, CallNtPowerInformation,
                GetSystemPowerStatus, POWER_REQUEST_TYPE, PowerClearRequest, PowerCreateRequest,
                PowerRequestDisplayRequired, PowerRequestSystemRequired, PowerSetRequest,
                SYSTEM_BATTERY_STATE, SYSTEM_POWER_STATUS, SystemBatteryState,
            },
            SystemServices::POWER_REQUEST_CONTEXT_VERSION,
            Threading::{POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0},
        },
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND, WM_NCDESTROY, WM_POWERBROADCAST,
            },
        },
    },
    core::PWSTR,
};

use super::{BatteryReading, IDLE_INHIBITION_REASON, format_windows_battery};

type WindowInhibitor = Rc<RefCell<Option<IdlePowerRequests>>>;

thread_local! {
    // Keep callback state on the window thread and route by the subclass ID. A callback
    // arriving after teardown cannot dereference stale data.
    static WINDOW_INHIBITORS: RefCell<HashMap<usize, WindowInhibitor>> = RefCell::default();
}

/// Owns the Windows idle requests and renews them after a manual suspend.
pub(super) struct DisplayPowerMonitor {
    hwnd: HWND,
    inhibitor: WindowInhibitor,
}

impl DisplayPowerMonitor {
    /// Blocks idle display and system sleep.
    pub(super) fn start(window: &Window, _: &BackgroundExecutor) -> Result<Self> {
        let native_handle = HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("could not read status window handle: {error}"))?;
        let RawWindowHandle::Win32(handle) = native_handle.as_raw() else {
            anyhow::bail!("status window has no Windows handle");
        };
        let hwnd = HWND(std::ptr::without_provenance_mut(
            handle.hwnd.get().cast_unsigned(),
        ));
        let inhibitor = Rc::new(RefCell::new(Some(IdlePowerRequests::acquire()?)));
        let id = Rc::as_ptr(&inhibitor).addr();

        // SAFETY: GPUI calls this on the thread that owns the live status window.
        let installed = unsafe { SetWindowSubclass(hwnd, Some(on_power_message), id, 0) };
        ensure!(
            installed.as_bool(),
            "could not observe status window power changes"
        );
        WINDOW_INHIBITORS.with_borrow_mut(|windows| windows.insert(id, Rc::clone(&inhibitor)));

        Ok(Self { hwnd, inhibitor })
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
                    Ok(PBT_APMSUSPEND) => {
                        inhibitor.replace(None);
                    }
                    Ok(PBT_APMRESUMEAUTOMATIC) => {
                        // Windows cancels active power requests on user-initiated sleep.
                        inhibitor.replace(None);
                        match IdlePowerRequests::acquire() {
                            Ok(requests) => {
                                inhibitor.replace(Some(requests));
                            }
                            Err(error) => log::warn!("could not renew idle inhibition: {error:#}"),
                        }
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

fn read_system_battery_state() -> Option<SYSTEM_BATTERY_STATE> {
    let mut battery = SYSTEM_BATTERY_STATE::default();
    let size = u32::try_from(std::mem::size_of_val(&battery)).ok()?;
    // SAFETY: SystemBatteryState takes no input and writes to this live output
    // buffer, whose byte size matches a SYSTEM_BATTERY_STATE.
    let status = unsafe {
        CallNtPowerInformation(
            SystemBatteryState,
            None,
            0,
            Some((&raw mut battery).cast()),
            size,
        )
    };
    (status.is_ok() && battery.BatteryPresent).then_some(battery)
}

/// Formats Windows battery percentage and power state for the dashboard.
pub(super) fn read_battery_status() -> Option<BatteryReading> {
    let power = match read_system_power_status() {
        Ok(power) => power,
        Err(error) => {
            log::debug!("could not read local battery status: {error}");
            return None;
        }
    };
    let label = format_windows_battery(
        power.ACLineStatus,
        power.BatteryFlag,
        power.BatteryLifePercent,
    )?;
    let mut reading = BatteryReading::new(label);
    reading.on_battery = power.ACLineStatus == 0;
    let battery = read_system_battery_state();
    reading.net_watts = battery
        .filter(|battery| battery.Rate != BATTERY_UNKNOWN_RATE)
        .map(|battery| f64::from(battery.Rate.cast_signed()) / 1000.0);
    if reading.on_battery {
        let estimate = if power.BatteryLifeTime != BATTERY_UNKNOWN_TIME {
            Some(power.BatteryLifeTime)
        } else {
            battery
                .filter(|battery| battery.EstimatedTime != BATTERY_UNKNOWN_TIME)
                .map(|battery| battery.EstimatedTime)
        };
        reading.remaining_seconds = estimate.map(u64::from);
    }
    Some(reading)
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
