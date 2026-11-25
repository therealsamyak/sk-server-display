//! Command-line startup and GPUI application composition.

use std::path::PathBuf;

use crate::{
    metrics::{
        build_status_feeds, default_status_config_path, load_status_config, status_state_path,
    },
    platform::DisplaySelector,
    profile::{export_profile, import_profile},
    wallpaper::find_style_wallpapers,
    wallpaper::{StatusCarousel, StatusPlayback},
};
use clap::Parser;
use gpui::{App, AppContext as _, Application, Entity, Global};
use time::UtcOffset;

use crate::{
    dashboard::{StatusDisplayApp, StatusWindowSettings, bind_status_keys},
    metrics::StatusBoard,
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

    /// Copy hosts, artwork, and slideshow settings to a new profile directory.
    #[arg(long, value_name = "DIRECTORY", conflicts_with = "import_profile")]
    export_profile: Option<PathBuf>,

    /// Restore a portable directory to a new profile. Use --config to set its location.
    #[arg(long, value_name = "DIRECTORY")]
    import_profile: Option<PathBuf>,
}

/// Global that keeps the status display controller alive for the whole run.
struct StatusDisplayRoot(#[allow(dead_code)] Entity<StatusDisplayApp>);

impl Global for StatusDisplayRoot {}

/// Starts the status display process.
pub(super) fn run_status_display() {
    crate::status_startup::record_process_start();
    // Read the local offset before GPUI starts threads. The `time` crate
    // refuses this lookup in a multithreaded process on Unix.
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let args = StatusDisplayArgs::parse();
    env_logger_init();

    if args.export_profile.is_some() || args.import_profile.is_some() {
        if let Err(error) = transfer_profile(&args) {
            log::error!("{error:#}");
            std::process::exit(2);
        }
        return;
    }

    let (config, feeds) = match load_status_config(args.config.as_deref())
        .and_then(|config| build_status_feeds(&config).map(|feeds| (config, feeds)))
    {
        Ok(loaded) => loaded,
        Err(error) => {
            log::error!("{error:#}");
            std::process::exit(2);
        }
    };
    let state_path = status_state_path(args.config.as_deref());
    let playback = match StatusPlayback::load_status_playback(state_path.as_deref()) {
        Ok(playback) => playback,
        Err(error) => {
            log::error!("{error:#}");
            std::process::exit(2);
        }
    };
    let wallpapers = config
        .preset_wallpapers
        .as_deref()
        .map(find_style_wallpapers);
    let carousel = StatusCarousel::new_status_carousel(playback, state_path, wallpapers);
    let settings = StatusWindowSettings {
        selector: args.display,
        start_fullscreen: !args.windowed,
        local_offset,
    };

    // No AccessKit adapter, as in Zed by default: a screen reader cannot
    // read the status display. The adapter adds a platform accessibility
    // connection to each window, and a wall display has no use for it.
    let application = Application::new_inaccessible(gpui_platform::current_platform(false));
    application.run(move |cx: &mut App| {
        crate::status_fonts::load_status_fonts(cx);
        bind_status_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let board = StatusBoard::new_status_board(feeds, cx);
        let carousel = cx.new(|_| carousel);
        let app = StatusDisplayApp::start_status_display(board, carousel, settings, cx);
        cx.set_global(StatusDisplayRoot(app));
    });
}

fn transfer_profile(args: &StatusDisplayArgs) -> anyhow::Result<()> {
    use anyhow::Context as _;

    if let Some(destination) = &args.export_profile {
        export_profile(args.config.as_deref(), destination)?;
        println!("Exported profile to {}", destination.display());
    }
    if let Some(source) = &args.import_profile {
        let config = args
            .config
            .clone()
            .or_else(default_status_config_path)
            .context("set the new configuration path with --config")?;
        import_profile(source, &config)?;
        println!("Imported profile to {}", config.display());
    }
    Ok(())
}

fn env_logger_init() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
}
