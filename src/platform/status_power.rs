//! Read local battery status and keep the fullscreen display awake on external power.

use std::time::Duration;

use gpui::{BackgroundExecutor, Context, Task, Window};

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
const BATTERY_POLL_INTERVAL: Duration = Duration::from_secs(5);
const AC_ONLY_BATTERY_LABEL: &str = "100% plugged in";

/// Maintains the battery label for the computer running the dashboard.
pub(crate) struct LocalBatteryStatus {
    text: Option<String>,
    _poll: Task<()>,
}

impl LocalBatteryStatus {
    /// Reads local battery status on the background executor.
    pub(crate) fn new_local_battery_status(cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async |status, cx| {
            let executor = cx.background_executor().clone();
            let mut reader = BatteryReader::default();
            loop {
                let (next_reader, text) = executor
                    .spawn(async move {
                        let text = reader.read_battery_status().await;
                        (reader, text)
                    })
                    .await;
                reader = next_reader;
                if status
                    .update(cx, |status, cx| {
                        if status.text != text {
                            status.text = text;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
                executor.timer(BATTERY_POLL_INTERVAL).await;
            }
        });
        Self {
            text: None,
            _poll: poll,
        }
    }

    /// Returns the most recent local battery label.
    pub(crate) fn battery_label(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

#[derive(Default)]
struct BatteryReader {
    #[cfg(target_os = "linux")]
    system_bus: Option<zbus::Connection>,
}

impl BatteryReader {
    async fn read_battery_status(&mut self) -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            platform::read_battery_status(&mut self.system_bus).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            platform::read_battery_status()
        }
    }
}

#[derive(Clone, Copy)]
enum BatteryPowerState {
    Charging,
    PluggedIn,
    OnBattery,
}

fn format_battery_status(percent: Option<f64>, state: Option<BatteryPowerState>) -> Option<String> {
    let percent = percent.filter(|value| (0.0..=100.0).contains(value));
    let state = state.map(|state| match state {
        BatteryPowerState::Charging => "charging",
        BatteryPowerState::PluggedIn => "plugged in",
        BatteryPowerState::OnBattery => "on battery",
    });
    match (percent, state) {
        (Some(percent), Some(state)) => Some(format!("{percent:.0}% {state}")),
        (Some(percent), None) => Some(format!("{percent:.0}%")),
        (None, Some(state)) => Some(state.into()),
        (None, None) => None,
    }
}

#[cfg(any(target_os = "windows", test))]
fn format_windows_battery(ac_line: u8, flags: u8, percent: u8) -> Option<String> {
    const CHARGING: u8 = 8;
    const NO_BATTERY: u8 = 128;
    const UNKNOWN_FLAGS: u8 = 255;
    let battery_known = flags != UNKNOWN_FLAGS;
    if battery_known && flags & NO_BATTERY != 0 {
        return (ac_line == 1).then(|| AC_ONLY_BATTERY_LABEL.into());
    }
    let state = match ac_line {
        _ if battery_known && flags & CHARGING != 0 => Some(BatteryPowerState::Charging),
        1 => Some(BatteryPowerState::PluggedIn),
        0 => Some(BatteryPowerState::OnBattery),
        _ => None,
    };
    format_battery_status(Some(f64::from(percent)), state)
}

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
    use super::{format_windows_battery, update_idle_inhibition};

    #[test]
    fn windows_battery_distinguishes_power_states() {
        let examples = [
            ((0, 4, 12), Some("12% on battery")),
            ((1, 8 | 2, 65), Some("65% charging")),
            ((1, 0, 80), Some("80% plugged in")),
            ((1, 128, 255), Some("100% plugged in")),
            ((0, 128, 255), None),
            ((1, 255, 255), Some("plugged in")),
            ((255, 255, 65), Some("65%")),
            ((255, 255, 255), None),
        ];
        for ((ac_line, flags, percent), expected) in examples {
            assert_eq!(
                format_windows_battery(ac_line, flags, percent).as_deref(),
                expected
            );
        }
    }

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
