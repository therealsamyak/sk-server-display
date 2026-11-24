//! Status state: the feeds, clock, window flags, and user actions of the
//! Rune status display.

use std::{path::PathBuf, sync::Arc, time::Instant};

use rune::{ImageCache, Key, KeyEvent, Tx, Waker};
use server_status_core::{
    status_feed::StatusFeed, status_freshness::FeedFreshness, status_model::FeedData,
};
use time::{OffsetDateTime, UtcOffset};

use crate::{display_placement::DisplayPlacement, status_poller::StatusRefresh};

/// Time that the window controls stay visible after the pointer moves.
const CONTROLS_VISIBLE_SECONDS: f64 = 3.0;

/// Interval of the clock tick that keeps the clock and the data ages current.
const STATUS_TICK_SECONDS: f64 = 1.0;

/// Status state: everything the status view shows.
pub struct StatusState {
    pub feeds: Vec<StatusFeed>,
    pub local_offset: UtcOffset,
    /// Wall-clock time of the last tick.
    pub now: OffsetDateTime,
    /// `true` while the status window has keyboard focus.
    pub window_focused: bool,
    /// `true` for a few seconds after the pointer moves over the window.
    pub controls_visible: bool,
    /// Background image, once the image cache has it.
    pub wallpaper: Option<rune::Image>,
    /// Label of the display that shows the window, such as `Display 1 of 2 (primary)`.
    pub display_label: String,
    pub display_count: usize,
    wallpaper_path: Option<PathBuf>,
    image_cache: Option<ImageCache>,
    started_at: Instant,
    controls_until: f64,
    next_tick: f64,
    refresh: StatusRefresh,
    placement: Arc<DisplayPlacement>,
}

/// Startup values of the status state.
pub struct StatusStateParts {
    pub feeds: Vec<StatusFeed>,
    pub local_offset: UtcOffset,
    pub wallpaper_path: Option<PathBuf>,
    pub display_label: String,
    pub display_count: usize,
    pub refresh: StatusRefresh,
    pub placement: Arc<DisplayPlacement>,
}

impl StatusState {
    /// Creates the status state. No poll runs until the window shows.
    pub fn new_status_state(parts: StatusStateParts) -> Self {
        Self {
            feeds: parts.feeds,
            local_offset: parts.local_offset,
            now: OffsetDateTime::now_utc(),
            window_focused: false,
            controls_visible: false,
            wallpaper: None,
            display_label: parts.display_label,
            display_count: parts.display_count,
            wallpaper_path: parts.wallpaper_path,
            image_cache: None,
            started_at: Instant::now(),
            controls_until: 0.0,
            next_tick: 0.0,
            refresh: parts.refresh,
            placement: parts.placement,
        }
    }

    /// Stores the result of one feed poll.
    pub fn record_feed_poll(&mut self, index: usize, result: anyhow::Result<FeedData>) {
        if let Some(feed) = self.feeds.get_mut(index) {
            feed.record_feed_poll(result);
        }
    }

    /// Freshness of one feed now.
    pub fn feed_freshness(&self, feed: &StatusFeed) -> FeedFreshness {
        feed.feed_freshness(self.started_at.elapsed())
    }

    /// `true` while this app keeps the window fullscreen.
    pub fn is_full_screen(&self) -> bool {
        self.placement.is_full_screen()
    }

    /// Stores the display label after the window moves.
    pub fn set_display_label(&mut self, label: String, display_count: usize) {
        self.display_label = label;
        self.display_count = display_count;
    }

    /// `AppSpec::setup`: starts the wallpaper decode on Rune's image workers.
    pub fn setup_status_images(&mut self, waker: Waker) {
        let cache = ImageCache::new(waker);
        self.wallpaper = self
            .wallpaper_path
            .as_ref()
            .map(|path| cache.file(path, 0.0, 1.0));
        self.image_cache = Some(cache);
    }

    /// `AppSpec::poll`: ticks the clock once a second and hides the window
    /// controls after their time. Returns `true` when the view must change.
    pub fn poll_status_clock(&mut self, clock: f64, tx: &mut Tx) -> bool {
        let mut changed = false;
        if self.controls_visible && clock >= self.controls_until {
            self.controls_visible = false;
            changed = true;
        }
        if clock >= self.next_tick {
            self.now = OffsetDateTime::now_utc();
            self.next_tick = clock + STATUS_TICK_SECONDS;
            changed = true;
        }
        if let Some(error) = self.wallpaper.as_ref().and_then(|image| image.error()) {
            log::error!("cannot show the wallpaper: {error}");
            self.wallpaper = None;
            changed = true;
        }
        tx.wake_at(self.next_tick);
        if self.controls_visible {
            tx.wake_at(self.controls_until);
        }
        changed
    }

    /// Shows the window controls for a few seconds after a pointer move.
    pub fn show_status_controls(&mut self, tx: &mut Tx) {
        self.controls_visible = true;
        self.controls_until = tx.clock() + CONTROLS_VISIBLE_SECONDS;
        tx.wake_at(self.controls_until);
    }

    /// `App::on_focus`: records keyboard focus changes for the focus alert.
    pub fn record_window_focus(&mut self, focused: bool, _: &mut Tx) {
        self.window_focused = focused;
    }

    /// `AppSpec::on_key`: the status display keyboard shortcuts.
    pub fn handle_status_key(&mut self, event: &KeyEvent, tx: &mut Tx) -> bool {
        match &event.key {
            Key::Function(11) => self.toggle_full_screen(tx),
            Key::Escape => self.exit_full_screen(tx),
            Key::Text(text) if text == "r" => self.refresh_status(),
            Key::Text(text) if text == "d" => self.next_status_display(),
            _ if event.is_command('q') => tx.quit(),
            _ => return false,
        }
        true
    }

    /// Enters or leaves fullscreen.
    pub fn toggle_full_screen(&mut self, tx: &mut Tx) {
        self.placement.set_full_screen(!self.is_full_screen());
        tx.toggle_full_screen();
    }

    /// Leaves fullscreen. The window stays open on the same display.
    pub fn exit_full_screen(&mut self, tx: &mut Tx) {
        if self.is_full_screen() {
            self.toggle_full_screen(tx);
        }
    }

    /// Polls every feed now.
    pub fn refresh_status(&self) {
        self.refresh.refresh_all_feeds();
    }

    /// Moves the window to the next display.
    pub fn next_status_display(&self) {
        self.placement.cycle_status_display(self.display_count);
    }
}
