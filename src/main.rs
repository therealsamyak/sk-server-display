//! Server status display: a full-screen, continuously updating status board.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod collector_glances;
mod collector_kuma;
mod collector_local;
mod collector_scrutiny;
mod collector_tailscale;
mod display_select;
#[cfg(target_os = "windows")]
mod display_windows;
mod status_board;
mod status_collector;
mod status_config;
// The physical display ranking runs on Windows only. Its tests run everywhere.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod status_display;
mod status_feed;
mod status_fonts;
mod status_format;
mod status_freshness;
mod status_graph;
mod status_model;
mod status_panel;
mod status_redraw;
mod status_startup;
mod status_style;
mod status_theme;
mod status_view;
mod status_wallpaper;
mod status_window;

use std::path::PathBuf;

use crate::{
    status_config::{build_status_feeds, load_status_config},
    status_display::DisplaySelector,
};
use clap::Parser;
use gpui::{App, Application, Entity, Global, KeyBinding};
use time::UtcOffset;

use crate::{
    status_board::StatusBoard,
    status_view::{
        ExitFullscreen, NextStatusDisplay, QuitStatusDisplay, RefreshStatus,
        STATUS_VIEW_KEY_CONTEXT, ToggleFullscreen,
    },
    status_window::{StatusDisplayApp, StatusWindowSettings},
};

/// Command-line options for the server status display.
#[derive(Debug, Parser)]
#[command(version, about)]
struct StatusDisplayArgs {
    /// Configuration file. Default: `server-status-display/config.toml` in
    /// the user configuration folder.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Display for the status window: `auto`, `primary`, or a number from 1.
    #[arg(long, default_value = "auto")]
    display: DisplaySelector,

    /// Open as a normal window instead of fullscreen.
    #[arg(long)]
    windowed: bool,

    /// Windows: let a click give the status window keyboard focus.
    #[arg(long)]
    allow_focus: bool,
}

/// Global that keeps the status display controller alive for the whole run.
struct StatusDisplayRoot(#[allow(dead_code)] Entity<StatusDisplayApp>);

impl Global for StatusDisplayRoot {}

fn main() {
    status_startup::record_process_start();
    // Read the local offset before GPUI starts threads. The `time` crate
    // refuses this lookup in a multithreaded process on Unix.
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let args = StatusDisplayArgs::parse();
    env_logger_init();

    let (config, feeds) = match load_status_config(args.config.as_deref())
        .and_then(|config| build_status_feeds(&config).map(|feeds| (config, feeds)))
    {
        Ok(loaded) => loaded,
        Err(error) => {
            log::error!("{error:#}");
            std::process::exit(2);
        }
    };
    status_style::set_status_style(config.style);
    let settings = StatusWindowSettings {
        wallpaper: config.wallpaper,
        selector: args.display,
        start_fullscreen: !args.windowed,
        allow_focus: args.allow_focus,
        local_offset,
    };

    // No AccessKit adapter, as in Zed by default: a screen reader cannot
    // read the status display. The adapter adds a platform accessibility
    // connection to each window, and a wall display has no use for it.
    let application = Application::new_inaccessible(gpui_platform::current_platform(false));
    application.run(move |cx: &mut App| {
        status_fonts::load_status_fonts(cx);
        bind_status_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let board = StatusBoard::new_status_board(feeds, cx);
        let app = StatusDisplayApp::start_status_display(board, settings, cx);
        cx.set_global(StatusDisplayRoot(app));
    });
}

/// Binds the status display keyboard shortcuts.
fn bind_status_keys(cx: &mut App) {
    let context = Some(STATUS_VIEW_KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("f11", ToggleFullscreen, context),
        KeyBinding::new("escape", ExitFullscreen, context),
        KeyBinding::new("r", RefreshStatus, context),
        KeyBinding::new("d", NextStatusDisplay, context),
        KeyBinding::new("ctrl-q", QuitStatusDisplay, context),
        KeyBinding::new("cmd-q", QuitStatusDisplay, context),
    ]);
}

fn env_logger_init() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
}
