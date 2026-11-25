//! Server status display: a full-screen, continuously updating status board.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod dashboard;
mod metrics;
mod platform;
mod profile;
mod status_fonts;
mod status_startup;
mod wallpaper;

fn main() {
    app::run_status_display();
}
