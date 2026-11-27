//! Read local battery status and inhibit idle sleep while requested.

use std::time::Duration;

use gpui::{Context, EventEmitter, Task};

#[cfg(target_os = "linux")]
#[path = "status_power/linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "status_power/macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "status_power/windows.rs"]
mod platform;

const IDLE_INHIBITION_REASON: &str = "Stay awake is enabled in Server Status Display";
const BATTERY_POLL_INTERVAL: Duration = Duration::from_secs(5);
const AC_ONLY_BATTERY_LABEL: &str = "100% plugged in";

/// Maintains the battery label for the computer running the dashboard.
pub(crate) struct LocalBatteryStatus {
    reading: Option<BatteryReading>,
    _poll: Task<()>,
}

/// The compact battery label changed.
pub(crate) struct BatteryLabelChanged;

impl EventEmitter<BatteryLabelChanged> for LocalBatteryStatus {}

/// Local battery status and optional estimates from the operating system.
#[derive(PartialEq)]
pub(crate) struct BatteryReading {
    pub label: String,
    pub net_watts: Option<f64>,
    pub remaining_seconds: Option<u64>,
    pub on_battery: bool,
}

impl BatteryReading {
    fn new(label: String) -> Self {
        Self {
            label,
            net_watts: None,
            remaining_seconds: None,
            on_battery: false,
        }
    }
}

impl LocalBatteryStatus {
    /// Reads local battery status on the background executor.
    pub(crate) fn new_local_battery_status(cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async |status, cx| {
            let executor = cx.background_executor().clone();
            let mut reader = BatteryReader::default();
            loop {
                let (next_reader, reading) = executor
                    .spawn(async move {
                        let text = reader.read_battery_status().await;
                        (reader, text)
                    })
                    .await;
                reader = next_reader;
                if status
                    .update(cx, |status, cx| {
                        if status.reading != reading {
                            let old_label = status.battery_label();
                            let new_label = reading.as_ref().map(|reading| reading.label.as_str());
                            if old_label != new_label {
                                cx.emit(BatteryLabelChanged);
                            }
                            status.reading = reading;
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
            reading: None,
            _poll: poll,
        }
    }

    /// Returns the most recent local battery label.
    pub(crate) fn battery_label(&self) -> Option<&str> {
        self.battery_reading().map(|reading| reading.label.as_str())
    }

    /// Returns the operating system's latest battery telemetry.
    pub(crate) fn battery_reading(&self) -> Option<&BatteryReading> {
        self.reading.as_ref()
    }
}

#[derive(Default)]
struct BatteryReader {
    #[cfg(target_os = "linux")]
    system_bus: Option<ashpd::zbus::Connection>,
}

impl BatteryReader {
    async fn read_battery_status(&mut self) -> Option<BatteryReading> {
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

/// State of the operating system's idle inhibition request.
#[derive(Clone, Copy, Default)]
pub(crate) enum IdlePowerState {
    #[default]
    Off,
    #[cfg(target_os = "linux")]
    Pending,
    Active,
    Failed,
}

/// Owns the app's idle inhibition request and exposes its status.
#[derive(Default)]
pub(crate) struct StatusPower {
    monitor: Option<platform::DisplayPowerMonitor>,
    state: IdlePowerState,
}

impl StatusPower {
    /// Starts or releases the app's idle inhibition request.
    pub(crate) fn set_keep_awake(&mut self, keep_awake: bool, cx: &mut Context<Self>) {
        if !keep_awake {
            self.monitor = None;
            self.state = IdlePowerState::Off;
            cx.notify();
        } else if self.monitor.is_none() {
            self.start_idle_inhibition(cx);
        }
    }

    /// Returns the latest idle inhibition status for the toolbar.
    pub(crate) fn idle_power_state(&self) -> IdlePowerState {
        self.state
    }

    #[cfg(target_os = "linux")]
    fn start_idle_inhibition(&mut self, cx: &mut Context<Self>) {
        self.state = IdlePowerState::Pending;
        self.monitor = Some(platform::DisplayPowerMonitor::start(cx));
        cx.notify();
    }

    #[cfg(not(target_os = "linux"))]
    fn start_idle_inhibition(&mut self, cx: &mut Context<Self>) {
        self.state = match platform::DisplayPowerMonitor::start(cx) {
            Ok(monitor) => {
                self.monitor = Some(monitor);
                IdlePowerState::Active
            }
            Err(error) => {
                log::warn!("idle inhibition unavailable: {error:#}");
                IdlePowerState::Failed
            }
        };
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::format_windows_battery;

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
}
