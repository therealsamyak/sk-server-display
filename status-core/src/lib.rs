//! Server status core: configuration, collectors, readings, freshness rules,
//! and display choice that the status display apps share. It has no UI code.

pub mod collector_glances;
pub mod collector_kuma;
pub mod collector_local;
pub mod collector_scrutiny;
pub mod collector_tailscale;
#[cfg(target_os = "windows")]
pub mod display_windows;
pub mod status_collector;
pub mod status_config;
pub mod status_display;
pub mod status_feed;
pub mod status_format;
pub mod status_freshness;
pub mod status_model;
pub mod status_startup;
