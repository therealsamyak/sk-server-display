//! Server status display: a full-screen, continuously updating status board.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod display_select;
#[cfg(target_os = "windows")]
mod display_windows;
mod status_board;
mod status_fonts;
mod status_view;
mod status_window;

use std::path::PathBuf;

use clap::Parser;
use gpui::{App, Entity, Global, KeyBinding};
use server_status_core::{
    status_config::{build_status_feeds, load_status_config},
    status_display::DisplaySelector,
};
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
    let settings = StatusWindowSettings {
        wallpaper: config.wallpaper,
        selector: args.display,
        start_fullscreen: !args.windowed,
        allow_focus: args.allow_focus,
        local_offset,
    };

    gpui_platform::application().run(move |cx: &mut App| {
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
