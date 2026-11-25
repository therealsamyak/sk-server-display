use std::{cell::RefCell, ffi::c_void, rc::Rc};

use anyhow::{Context as _, Result, ensure};
use gpui::{BackgroundExecutor, Window};
use objc2_core_foundation::{
    CFRetained, CFRunLoop, CFRunLoopSource, CFString, kCFRunLoopCommonModes,
};
use objc2_io_kit::{
    IOPMAssertionCreateWithName, IOPMAssertionID, IOPMAssertionRelease, IOPSCopyPowerSourcesInfo,
    IOPSCreateLimitedPowerNotification, IOPSGetProvidingPowerSourceType, kIOPMAssertionLevelOn,
};

use super::{IDLE_INHIBITION_REASON, update_idle_inhibition};

type IdleAssertions = (IdleAssertion, IdleAssertion);

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
        // SAFETY: the snapshot is the value from IOPSCopyPowerSourcesInfo.
        let source = unsafe { IOPSGetProvidingPowerSourceType(Some(&snapshot)) };
        source.is_some_and(|source| *source == *CFString::from_str("AC Power"))
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
