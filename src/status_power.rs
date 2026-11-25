//! Keep the display and system awake on external power during display mode.

use gpui::{BackgroundExecutor, Window};

#[cfg(target_os = "linux")]
#[path = "status_power/linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "status_power/macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "status_power/windows.rs"]
mod platform;

const IDLE_INHIBITION_REASON: &str = "Server status display is fullscreen";

/// Owns idle inhibition for the visible fullscreen status window.
#[derive(Default)]
pub(crate) struct StatusPower(Option<platform::DisplayPowerMonitor>);

impl StatusPower {
    /// Starts or stops monitoring external power as the window enters or leaves display mode.
    pub(crate) fn set_display_mode(
        &mut self,
        display_mode: bool,
        window: &Window,
        executor: &BackgroundExecutor,
    ) {
        if !display_mode {
            self.0 = None;
        } else if self.0.is_none() {
            match platform::DisplayPowerMonitor::start(window, executor) {
                Ok(monitor) => self.0 = Some(monitor),
                Err(error) => log::warn!("idle inhibition unavailable: {error:#}"),
            }
        }
    }
}

fn update_idle_inhibition<T>(
    guard: &mut Option<T>,
    external_power: bool,
    acquire: impl FnOnce() -> anyhow::Result<T>,
) {
    if !external_power {
        *guard = None;
    } else if guard.is_none() {
        match acquire() {
            Ok(inhibitor) => *guard = Some(inhibitor),
            Err(error) => log::warn!("idle inhibition unavailable: {error:#}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::update_idle_inhibition;

    #[test]
    fn battery_power_does_not_acquire_idle_inhibition() {
        let mut guard = None;
        update_idle_inhibition(&mut guard, false, || Ok(()));
        assert!(guard.is_none());
    }

    #[test]
    fn battery_power_releases_existing_idle_inhibition() {
        use std::rc::Rc;

        let token = Rc::new(());
        let mut guard = Some(Rc::clone(&token));
        update_idle_inhibition(&mut guard, false, || Ok(Rc::clone(&token)));
        assert_eq!(Rc::strong_count(&token), 1);
    }

    #[test]
    fn external_power_acquires_idle_inhibition() {
        let mut guard = None;
        update_idle_inhibition(&mut guard, true, || Ok(()));
        assert!(guard.is_some());
    }

    #[test]
    fn external_power_preserves_existing_idle_inhibition() {
        let mut guard = Some(1);
        update_idle_inhibition(&mut guard, true, || Ok(2));
        assert_eq!(guard, Some(1));
    }
}
