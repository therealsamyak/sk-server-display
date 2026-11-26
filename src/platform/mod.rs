mod display_select;
#[cfg(target_os = "windows")]
mod display_windows;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod status_display;
mod status_power;
pub(crate) mod status_release;
pub(crate) mod status_update;

pub(crate) use display_select::{DisplayChoice, choose_status_display, ordered_status_displays};
pub(crate) use status_display::DisplaySelector;
pub(crate) use status_power::{LocalBatteryStatus, StatusPower};
