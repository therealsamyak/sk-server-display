//! Server status display: a full-screen, continuously updating status board.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod display_select;
#[cfg(target_os = "windows")]
mod display_windows;
mod status_board;
mod status_demo;
mod status_fonts;
mod status_format;
mod status_freshness;
mod status_snapshot;
mod status_source;
mod status_view;
mod status_window;

use std::time::Duration;

use clap::Parser;
use gpui::{App, Entity, Global, KeyBinding};
use time::UtcOffset;

use crate::{
    display_select::DisplaySelector,
    status_board::StatusBoard,
    status_demo::{DemoScenario, DemoStatusSource},
    status_freshness::FreshnessPolicy,
    status_source::StatusSource,
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
    /// Status snapshot URL, or `demo` for built-in sample data.
    #[arg(long, default_value = "demo")]
    source: String,

    /// Seconds between two status polls.
    #[arg(long, default_value_t = 2.0)]
    interval: f64,

    /// Display for the status window: `auto`, `primary`, or a number from 1.
    #[arg(long, default_value = "auto")]
    display: DisplaySelector,

    /// Open as a normal window instead of fullscreen.
    #[arg(long)]
    windowed: bool,

    /// Windows: let a click give the status window keyboard focus.
    #[arg(long)]
    allow_focus: bool,

    /// Demo data scenario.
    #[arg(long, value_enum, default_value_t = DemoScenario::Mixed)]
    demo_scenario: DemoScenario,

    /// Demo only: make polls fail after this many seconds.
    #[arg(long)]
    demo_outage_after: Option<f64>,
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

    let policy = FreshnessPolicy::from_poll_interval(Duration::from_secs_f64(args.interval));
    let source = if args.source == "demo" {
        StatusSource::Demo(DemoStatusSource::new(
            args.demo_scenario,
            args.demo_outage_after.map(Duration::from_secs_f64),
        ))
    } else {
        StatusSource::http_status_source(args.source.clone(), &policy)
    };
    let settings = StatusWindowSettings {
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
        let board = StatusBoard::new_status_board(source, policy, cx);
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
