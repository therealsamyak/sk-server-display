use std::{cell::RefCell, rc::Rc};

use anyhow::{Context as _, Result};
use gpui::{App, Subscription};
use windows::{
    Win32::{
        Foundation::HANDLE,
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
    },
    core::{Owned, PWSTR},
};

use super::{BatteryReading, IDLE_INHIBITION_REASON, format_windows_battery};

/// Owns the Windows idle requests and renews them after a manual suspend.
pub(super) struct DisplayPowerMonitor {
    _sleep: Subscription,
    _wake: Subscription,
    _requests: Rc<RefCell<Option<IdlePowerRequests>>>,
}

impl DisplayPowerMonitor {
    /// Blocks idle display and system sleep.
    pub(super) fn start(cx: &App) -> Result<Self> {
        let requests = Rc::new(RefCell::new(Some(IdlePowerRequests::acquire()?)));
        let sleep = cx.on_system_sleep({
            let requests = Rc::downgrade(&requests);
            move |_| {
                if let Some(requests) = requests.upgrade() {
                    requests.replace(None);
                }
            }
        });
        let wake = cx.on_system_wake({
            let requests = Rc::downgrade(&requests);
            move |_| {
                let Some(requests) = requests.upgrade() else {
                    return;
                };
                // Windows cancels active requests on user-initiated sleep.
                requests.replace(None);
                match IdlePowerRequests::acquire() {
                    Ok(active) => {
                        requests.replace(Some(active));
                    }
                    Err(error) => log::warn!("could not renew idle inhibition: {error:#}"),
                }
            }
        });
        Ok(Self {
            _sleep: sleep,
            _wake: wake,
            _requests: requests,
        })
    }
}

#[expect(
    unsafe_code,
    reason = "GetSystemPowerStatus supplies AC and charge flags"
)]
fn read_system_power_status() -> windows::core::Result<SYSTEM_POWER_STATUS> {
    let mut power = SYSTEM_POWER_STATUS::default();
    // Temporary unsafe for AC and charge flags. Zed uses the same status API.
    // https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/platform.rs#L237-L244
    // SAFETY: `power` is a writable, correctly typed status buffer.
    unsafe { GetSystemPowerStatus(&mut power) }?;
    Ok(power)
}

#[expect(
    unsafe_code,
    reason = "SystemBatteryState supplies signed charge rate and time"
)]
fn read_system_battery_state() -> Option<SYSTEM_BATTERY_STATE> {
    let mut battery = SYSTEM_BATTERY_STATE::default();
    let size = u32::try_from(std::mem::size_of_val(&battery)).ok()?;
    // Temporary unsafe for charge rate and time. Sysinfo uses this API for CPU power.
    // https://github.com/GuillaumeGomez/sysinfo/blob/847a39249bdc82215f6a6e4f2ae5b1c7bdc969ac/src/windows/cpu.rs#L468-L479
    // SAFETY: no input is required. The writable output buffer matches SystemBatteryState.
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
    // Drop the handle before the UTF-16 reason it borrows.
    handle: Owned<HANDLE>,
    active: usize,
    _reason: Vec<u16>,
}

impl IdlePowerRequests {
    #[expect(unsafe_code, reason = "GPUI has no Windows display idle request")]
    fn acquire() -> Result<Self> {
        // Temporary unsafe for display idle inhibition. Zed uses this API for system idle.
        // https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/platform.rs#L138-L169
        let mut reason: Vec<u16> = IDLE_INHIBITION_REASON.encode_utf16().chain([0]).collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason.as_mut_ptr()),
            },
        };
        let mut handle = Owned::<HANDLE>::default();
        // SAFETY: this simple-string context has a UTF-16 reason that outlives the handle.
        *handle =
            unsafe { PowerCreateRequest(&context) }.context("could not create power request")?;
        let mut requests = Self {
            handle,
            active: 0,
            _reason: reason,
        };
        for request in IDLE_REQUESTS {
            // SAFETY: the live handle is owned by `requests` and the request type is valid.
            unsafe { PowerSetRequest(*requests.handle, request) }
                .context("could not set idle power request")?;
            requests.active += 1;
        }
        Ok(requests)
    }
}

impl Drop for IdlePowerRequests {
    #[expect(
        unsafe_code,
        reason = "Win32 requires clearing each active idle request"
    )]
    fn drop(&mut self) {
        // Temporary unsafe to balance active requests. Zed uses the same teardown.
        // https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/platform.rs#L172-L180
        for request in IDLE_REQUESTS.into_iter().take(self.active) {
            // SAFETY: every cleared request was set on this live handle.
            if let Err(error) = unsafe { PowerClearRequest(*self.handle, request) } {
                log::warn!("could not clear idle power request: {error}");
            }
        }
    }
}
