use anyhow::Result;
use gpui::{ActivityGuard, App};
use gpui_apple::AppleActivity;
use objc2_core_foundation::{CFBoolean, CFNumber, CFString, CFType};
use objc2_foundation::NSActivityOptions;
use objc2_io_kit::{
    IOPSCopyPowerSourcesInfo, IOPSCopyPowerSourcesList, IOPSGetPowerSourceDescription,
    IOPSGetProvidingPowerSourceType, kIOPSCurrentCapacityKey, kIOPSCurrentKey,
    kIOPSInternalBatteryType, kIOPSIsChargingKey, kIOPSIsPresentKey, kIOPSMaxCapacityKey,
    kIOPSTimeToEmptyKey, kIOPSTypeKey, kIOPSVoltageKey,
};

use super::{
    AC_ONLY_BATTERY_LABEL, BatteryPowerState, BatteryReading, IDLE_INHIBITION_REASON,
    format_battery_status,
};

/// Reads the built-in battery from a macOS power-source snapshot.
#[expect(
    unsafe_code,
    reason = "IOKit exposes built-in battery fields through opaque snapshots"
)]
pub(super) fn read_battery_status() -> Option<BatteryReading> {
    // Temporary unsafe for charge rate and time. Uzu uses the same IOKit snapshot APIs.
    // https://github.com/trymirai/uzu/blob/8fd73f041d6971bdae84e846c80fadb3e09b38c5/crates/legacy/keisoku/src/sys/battery.rs#L9-L29
    let snapshot = IOPSCopyPowerSourcesInfo()?;
    // SAFETY: only the live IOKit snapshot is passed to its list accessor.
    let sources = unsafe { IOPSCopyPowerSourcesList(Some(&snapshot)) }?;
    // SAFETY: IOKit guarantees that each source handle is a CFType.
    let sources = unsafe { sources.cast_unchecked::<CFType>() };
    // Temporary unsafe to distinguish AC, battery, and UPS. Hammerspoon uses this API.
    // https://github.com/Hammerspoon/hammerspoon/blob/f3928953b584a91ed74d0402ddd6dfb2f9b97629/extensions/battery/libbattery.m#L64-L72
    // SAFETY: the live snapshot came from IOPSCopyPowerSourcesInfo.
    let state = unsafe { IOPSGetProvidingPowerSourceType(Some(&snapshot)) }
        .as_deref()
        .and_then(mac_power_state);
    let battery_type = CFString::from_str(kIOPSInternalBatteryType.to_str().ok()?);
    for source in sources.iter() {
        // SAFETY: the source belongs to the same live snapshot.
        let description = unsafe { IOPSGetPowerSourceDescription(Some(&snapshot), Some(&source)) }?;
        // SAFETY: IOKit guarantees CFString keys and CFType values.
        let description = unsafe { description.cast_unchecked::<CFString, CFType>() };
        let lookup =
            |key: &std::ffi::CStr| description.get(&CFString::from_str(key.to_str().ok()?));
        let is_internal = lookup(kIOPSTypeKey)
            .and_then(|kind| kind.downcast::<CFString>().ok())
            .is_some_and(|kind| *kind == *battery_type);
        if !is_internal {
            continue;
        }
        let flag = |key| lookup(key).and_then(|value| value.downcast::<CFBoolean>().ok());
        if flag(kIOPSIsPresentKey).is_some_and(|flag| !flag.as_bool()) {
            continue;
        }
        let capacity = |key| lookup(key)?.downcast::<CFNumber>().ok()?.as_f64();
        let percent = capacity(kIOPSCurrentCapacityKey)
            .zip(capacity(kIOPSMaxCapacityKey).filter(|max| *max > 0.0))
            .map(|(current, max)| current / max * 100.0);
        let state = if flag(kIOPSIsChargingKey).is_some_and(|flag| flag.as_bool()) {
            Some(BatteryPowerState::Charging)
        } else {
            state
        };
        let mut reading = BatteryReading::new(format_battery_status(percent, state)?);
        reading.on_battery = matches!(state, Some(BatteryPowerState::OnBattery));
        reading.net_watts = capacity(kIOPSCurrentKey)
            .zip(capacity(kIOPSVoltageKey).filter(|voltage| *voltage > 0.0))
            .map(|(current, voltage)| (current * voltage) / 1_000_000.0)
            .filter(|watts| watts.is_finite());
        if reading.on_battery {
            reading.remaining_seconds = lookup(kIOPSTimeToEmptyKey)
                .and_then(|minutes| minutes.downcast::<CFNumber>().ok()?.as_i64())
                .and_then(|minutes| u64::try_from(minutes).ok()?.checked_mul(60));
        }
        return Some(reading);
    }
    matches!(state, Some(BatteryPowerState::PluggedIn))
        .then(|| BatteryReading::new(AC_ONLY_BATTERY_LABEL.into()))
}

fn mac_power_state(source: &CFString) -> Option<BatteryPowerState> {
    if source == &*CFString::from_str("AC Power") {
        Some(BatteryPowerState::PluggedIn)
    } else if source == &*CFString::from_str("Battery Power") {
        Some(BatteryPowerState::OnBattery)
    } else {
        None
    }
}

/// Owns the macOS activity that blocks idle sleep.
pub(super) struct DisplayPowerMonitor {
    _activity: ActivityGuard,
}

impl DisplayPowerMonitor {
    /// Blocks idle display and system sleep.
    pub(super) fn start(_: &App) -> Result<Self> {
        let options = NSActivityOptions::IdleDisplaySleepDisabled
            | NSActivityOptions::IdleSystemSleepDisabled;
        Ok(Self {
            _activity: AppleActivity::begin(IDLE_INHIBITION_REASON, options),
        })
    }
}
