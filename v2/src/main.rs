//! Server status display, v2: the status board built with Rune.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod display_placement;
mod status_graphs;
mod status_poller;
mod status_state;
mod status_theme;
mod status_view;

use std::path::PathBuf;

use clap::Parser;
use rune::{App, AppSpec, WindowOptions, WindowSpec};
use server_status_core::{
    status_config::{build_status_feeds, load_status_config},
    status_display::DisplaySelector,
    status_feed::StatusFeed,
};
use time::UtcOffset;

use crate::{
    display_placement::{
        DisplayPlacement, choose_display_option, rune_display_options, start_placement_watcher,
        windowed_position,
    },
    status_poller::{new_status_poll_jobs, start_status_pollers},
    status_state::{StatusState, StatusStateParts},
    status_view::{STATUS_WINDOW_SIZE, status_view},
};

/// Bundled status display fonts: IBM Plex Sans for the clock, IBM Plex Mono
/// for the panels. Text looks the same on Windows, macOS, and Linux.
const STATUS_FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/ibm-plex-sans/IBMPlexSans-Light.ttf"),
    include_bytes!("../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../assets/fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/ibm-plex-mono/IBMPlexMono-SemiBold.ttf"),
];

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

fn main() {
    // Read the local offset before any thread starts. The `time` crate
    // refuses this lookup in a multithreaded process on Unix.
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    // Rune takes its own flags (`--snap`, `--size`, and the other headless
    // tools) and leaves the rest for the app.
    let program = std::env::args().next().unwrap_or_default();
    let args = StatusDisplayArgs::parse_from(
        std::iter::once(program).chain(rune::extra_args().iter().cloned()),
    );
    env_logger_init();

    let (config, specs) = match load_status_config(args.config.as_deref())
        .and_then(|config| build_status_feeds(&config).map(|specs| (config, specs)))
    {
        Ok(loaded) => loaded,
        Err(error) => {
            log::error!("{error:#}");
            std::process::exit(2);
        }
    };

    rune::run_windows(move |displays| {
        let startup_displays = rune_display_options(displays);
        let choice = choose_display_option(args.display, &startup_displays);
        let display_label = choice
            .as_ref()
            .map_or_else(|| "No display".to_string(), |choice| choice.label.clone());
        let full_screen = !args.windowed;
        let (placement, nudges) =
            DisplayPlacement::new_display_placement(args.display, choice.clone(), full_screen);
        let poll_feeds: Vec<_> = specs
            .iter()
            .map(|spec| (spec.collector.clone(), spec.poll_interval))
            .collect();
        let (refresh, jobs) = new_status_poll_jobs(&poll_feeds);

        let spec = AppSpec {
            fonts: STATUS_FONTS,
            families: ("IBM Plex Sans", "IBM Plex Mono"),
            setup: Some(StatusState::setup_status_images),
            poll: Some(StatusState::poll_status_clock),
            on_key: Some(StatusState::handle_status_key),
            ..AppSpec::new(
                "Server status",
                STATUS_WINDOW_SIZE,
                || unreachable!("App::init builds the status state"),
                status_view,
            )
        };
        let app = App::new(spec);
        let handle = app.handle();
        let state_parts = StatusStateParts {
            feeds: specs.into_iter().map(StatusFeed::new_status_feed).collect(),
            local_offset,
            wallpaper_path: config.wallpaper,
            display_label,
            display_count: startup_displays.len(),
            refresh,
            placement: placement.clone(),
        };
        let mut startup = Some((jobs, nudges, startup_displays));
        let allow_focus = args.allow_focus;
        let app = app
            .init(move || StatusState::new_status_state(state_parts))
            .window_options(WindowOptions {
                position: choice.map(|choice| windowed_position(&choice.display)),
                ..WindowOptions::default()
            })
            .full_screen(full_screen)
            .open_inactive()
            .on_focus(StatusState::record_window_focus)
            .on_window(move |window| {
                if !allow_focus {
                    apply_no_activate_style(window);
                }
            })
            .on_ready(move |_| {
                // `on_ready` runs once for each native window creation.
                // The poll threads and the watcher start only once.
                let Some((jobs, nudges, startup_displays)) = startup.take() else {
                    return;
                };
                start_status_pollers(&handle, jobs);
                start_placement_watcher(
                    handle.clone(),
                    placement.clone(),
                    startup_displays,
                    nudges,
                );
            });
        vec![WindowSpec::from_app("status", app)]
    });
}

/// Windows: adds `WS_EX_NOACTIVATE` to the status window, so a click does
/// not make it the foreground window and Moonlight input stays in the game.
#[cfg(target_os = "windows")]
fn apply_no_activate_style(window: &rune::WindowHandle) {
    use raw_window_handle::RawWindowHandle;
    let applied = window.with_raw_handle(|handle| {
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            server_status_core::display_windows::apply_no_activate_style(handle.hwnd.get());
        }
    });
    if let Err(error) = applied {
        log::warn!("cannot set the no-activate window style: {error}");
    }
}

/// macOS and Linux have no no-activate window style. `open_inactive` keeps
/// focus away on the first open only.
#[cfg(not(target_os = "windows"))]
fn apply_no_activate_style(_: &rune::WindowHandle) {}

fn env_logger_init() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
}
