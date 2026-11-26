use std::{cell::RefCell, ffi::c_void, rc::Rc};

use anyhow::{Context as _, Result, ensure};
use gpui::{BackgroundExecutor, Window};
use objc2_core_foundation::{
    CFBoolean, CFNumber, CFRetained, CFRunLoop, CFRunLoopSource, CFString, CFType,
    kCFRunLoopCommonModes,
};
use objc2_io_kit::{
    IOPMAssertionCreateWithName, IOPMAssertionID, IOPMAssertionRelease, IOPSCopyPowerSourcesInfo,
    IOPSCopyPowerSourcesList, IOPSCreateLimitedPowerNotification, IOPSGetPowerSourceDescription,
    IOPSGetProvidingPowerSourceType, kIOPMAssertionLevelOn, kIOPSCurrentCapacityKey,
    kIOPSInternalBatteryType, kIOPSIsChargingKey, kIOPSIsPresentKey, kIOPSMaxCapacityKey,
    kIOPSTypeKey,
};

use super::{
    AC_ONLY_BATTERY_LABEL, BatteryPowerState, IDLE_INHIBITION_REASON, format_battery_status,
    update_idle_inhibition,
};

type IdleAssertions = (IdleAssertion, IdleAssertion);

/// Reads the built-in battery from a macOS power-source snapshot.
pub(super) fn read_battery_status() -> Option<String> {
    let snapshot = IOPSCopyPowerSourcesInfo()?;
    // SAFETY: this is the snapshot returned by IOPSCopyPowerSourcesInfo.
    let sources = unsafe { IOPSCopyPowerSourcesList(Some(&snapshot)) }?;
    // SAFETY: IOKit documents the returned array as containing CFType objects.
    let sources = unsafe { sources.cast_unchecked::<CFType>() };
    let state = mac_power_state(&snapshot);
    let battery_type = CFString::from_str(kIOPSInternalBatteryType.to_str().ok()?);
    for source in sources.iter() {
        // SAFETY: the source is an entry from this snapshot's power-source array.
        let description = unsafe { IOPSGetPowerSourceDescription(Some(&snapshot), Some(&source)) }?;
        // SAFETY: IOKit describes power sources with CFString keys and CF objects.
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
        return format_battery_status(percent, state);
    }
    matches!(state, Some(BatteryPowerState::PluggedIn)).then(|| AC_ONLY_BATTERY_LABEL.into())
}

fn mac_power_state(snapshot: &CFType) -> Option<BatteryPowerState> {
    // SAFETY: the snapshot is the value from IOPSCopyPowerSourcesInfo.
    let source = unsafe { IOPSGetProvidingPowerSourceType(Some(snapshot)) }?;
    if *source == *CFString::from_str("AC Power") {
        Some(BatteryPowerState::PluggedIn)
    } else if *source == *CFString::from_str("Battery Power") {
        Some(BatteryPowerState::OnBattery)
    } else {
        None
    }
}

/// Owns the macOS idle assertions and power source notification.
pub(super) struct DisplayPowerMonitor {
    source: CFRetained<CFRunLoopSource>,
    _assertions: Rc<RefCell<Option<IdleAssertions>>>,
}

impl DisplayPowerMonitor {
    /// Watches external power on the status window run loop.
    pub(super) fn start(_: &Window, _: &BackgroundExecutor) -> Result<Self> {
        let run_loop = CFRunLoop::current().context("could not find the status window run loop")?;
        let assertions = Rc::new(RefCell::new(None));
        let context = Rc::as_ptr(&assertions).cast_mut().cast();
        // SAFETY: `assertions` stays at a stable address until this source is invalidated.
        // The source is added only to this window's run loop, on the same thread as Drop.
        let source = unsafe { IOPSCreateLimitedPowerNotification(Some(on_power_change), context) }
            .context("could not observe changes between external and battery power")?;
        // SAFETY: this Core Foundation constant names the run loop's common modes.
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        refresh_idle_inhibition(&assertions);
        Ok(Self {
            source,
            _assertions: assertions,
        })
    }
}

impl Drop for DisplayPowerMonitor {
    fn drop(&mut self) {
        self.source.invalidate();
    }
}

unsafe extern "C-unwind" fn on_power_change(context: *mut c_void) {
    // SAFETY: this pointer references the guard's live, same-run-loop assertion state.
    let assertions = unsafe { &*context.cast::<RefCell<Option<IdleAssertions>>>() };
    refresh_idle_inhibition(assertions);
}

fn refresh_idle_inhibition(assertions: &RefCell<Option<IdleAssertions>>) {
    let on_ac = IOPSCopyPowerSourcesInfo().is_some_and(|snapshot| {
        matches!(
            mac_power_state(&snapshot),
            Some(BatteryPowerState::PluggedIn)
        )
    });
    update_idle_inhibition(&mut assertions.borrow_mut(), on_ac, || {
        Ok((
            IdleAssertion::acquire("PreventUserIdleDisplaySleep")?,
            IdleAssertion::acquire("PreventUserIdleSystemSleep")?,
        ))
    });
}

struct IdleAssertion(IOPMAssertionID);

impl IdleAssertion {
    fn acquire(assertion_type: &str) -> Result<Self> {
        let mut id = 0;
        // SAFETY: the non-null strings and writable assertion ID live through the call.
        let status = unsafe {
            IOPMAssertionCreateWithName(
                Some(&CFString::from_str(assertion_type)),
                kIOPMAssertionLevelOn,
                Some(&CFString::from_str(IDLE_INHIBITION_REASON)),
                &mut id,
            )
        };
        ensure!(
            status == 0,
            "could not acquire macOS idle assertion ({status:#x})"
        );
        Ok(Self(id))
    }
}

impl Drop for IdleAssertion {
    fn drop(&mut self) {
        let status = IOPMAssertionRelease(self.0);
        if status != 0 {
            log::warn!("could not release macOS idle assertion ({status:#x})");
        }
    }
}
