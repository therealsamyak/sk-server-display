//! Status view: the full-screen GPUI layout for the status display.
//!
//! The root view draws the wallpaper, the clock, the alerts, and the window
//! controls. Each feed panel is a cached child view (see `status_panel`), so
//! a root redraw for the clock reuses the panels as they are.

mod status_view_layout;
mod status_wallpaper_view;

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, IntoElement,
    KeyBinding, MouseButton, ObjectFit, Render, StyledImage as _, Subscription, WeakEntity, Window,
    actions, div, img, prelude::*, px, relative,
};
use time::{OffsetDateTime, UtcOffset};

use crate::{
    metrics::{StatusBoard, StatusFeed},
    platform::StatusPower,
    status_fonts::STATUS_MONO_FONT_FAMILY,
    status_startup::{StartupMilestone, log_startup_milestone},
    wallpaper::{StatusCarousel, StyleArrangement},
};

use super::{
    status_format::status_clock_text_valid_for,
    status_panel::{FeedDetailsClosed, FeedPanelView},
    status_redraw::StatusRedrawTimer,
    status_theme::{Scale, StatusLayout, StatusTheme},
    status_window::StatusDisplayApp,
};
use status_view_layout::{alert_row, area_shade, control_button, feed_alert, status_clock};
use status_wallpaper_view::{PickerSession, StatusWallpaper, WallpaperPreview};

actions!(
    status_display,
    [
        /// Enters or leaves fullscreen on the status display.
        ToggleFullscreen,
        /// Leaves fullscreen and shows the status window as a normal window.
        ExitFullscreen,
        /// Quits the status display app.
        QuitStatusDisplay,
        /// Polls every status feed now.
        RefreshStatus,
        /// Moves the status window to the next display.
        NextStatusDisplay,
    ]
);

/// Key context of the status view, used by the status display key bindings.
const STATUS_VIEW_KEY_CONTEXT: &str = "StatusDisplay";

/// Binds the status display keyboard shortcuts.
pub(crate) fn bind_status_keys(cx: &mut App) {
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

/// Time that the window controls stay visible after the pointer moves.
const CONTROLS_VISIBLE_DURATION: Duration = Duration::from_secs(3);

/// Per-window settings of the status view.
pub struct StatusViewSettings {
    /// Display description, such as `Display 1 of 2 (primary)`.
    pub display_label: String,
    pub local_offset: UtcOffset,
}

/// Status view: clock, alerts, host panels, services, disks, and tailnet.
pub struct StatusView {
    board: Entity<StatusBoard>,
    carousel: Entity<StatusCarousel>,
    controller: WeakEntity<StatusDisplayApp>,
    settings: StatusViewSettings,
    focus_handle: FocusHandle,
    /// One panel view for each feed, in feed order.
    panels: Vec<Entity<FeedPanelView>>,
    /// Number of host panels that share the host column.
    host_rows: usize,
    /// Devices with a host card. The tailnet panel leaves them out.
    carded: Vec<String>,
    wallpaper: StatusWallpaper,
    picker: Option<PickerSession>,
    preview: Option<WallpaperPreview>,
    preview_error: Option<String>,
    details: Option<FeedDetailsSession>,
    pointer_pressed: bool,
    controls_latched: bool,
    power: StatusPower,
    pointer_moved_at: Option<Instant>,
    first_frame_requested: bool,
    redraw: StatusRedrawTimer,
    _subscriptions: Vec<Subscription>,
}

struct FeedDetailsSession {
    index: usize,
    panel: Entity<FeedPanelView>,
    _close: Subscription,
}

impl StatusView {
    /// Creates the status view for one window.
    pub fn new_status_view(
        settings: StatusViewSettings,
        board: Entity<StatusBoard>,
        carousel: Entity<StatusCarousel>,
        controller: WeakEntity<StatusDisplayApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        // The board emits events and does not notify, so the root view draws
        // again only for its own changes and for panel changes.
        let view = cx.weak_entity();
        let subscriptions = vec![
            window.observe_window_visibility(move |_, window, cx| {
                if let Err(error) = view.update(cx, |view, cx| {
                    view.update_display_power(window, cx);
                    if !window.is_visible() {
                        view.pointer_pressed = false;
                    }
                    cx.notify();
                }) {
                    log::debug!("status window closed: {error:#}");
                }
            }),
            cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() {
                    view.pointer_pressed = false;
                }
                cx.notify();
            }),
        ];
        let feeds = board.read(cx).status_feeds();
        let is_host = |feed: &&StatusFeed| feed.spec.collector.is_host_collector();
        let host_count = feeds.iter().filter(is_host).count();
        // Up to two hosts get one column each. More hosts share one column.
        let host_rows = if host_count <= 2 { 1 } else { host_count };
        // Devices with a host card do not repeat in the tailnet list.
        let carded: Vec<String> = feeds
            .iter()
            .filter_map(|feed| feed.spec.collector.tailnet_device_name().map(str::to_owned))
            .collect();
        let panels = (0..feeds.len())
            .map(|index| {
                let board = board.clone();
                let carded = carded.clone();
                cx.new(|cx| FeedPanelView::new_feed_panel(board, index, carded, cx))
            })
            .collect();
        Self {
            board,
            carousel,
            controller,
            settings,
            focus_handle,
            panels,
            host_rows,
            carded,
            wallpaper: StatusWallpaper::default(),
            picker: None,
            preview: None,
            preview_error: None,
            details: None,
            pointer_pressed: false,
            controls_latched: false,
            power: StatusPower::default(),
            pointer_moved_at: None,
            first_frame_requested: false,
            redraw: StatusRedrawTimer::default(),
            _subscriptions: subscriptions,
        }
    }

    fn update_display_power(&mut self, window: &Window, cx: &Context<Self>) {
        self.power.set_display_mode(
            window.is_fullscreen() && window.is_visible(),
            window,
            cx.background_executor(),
        );
    }

    fn open_feed_details(&mut self, index: usize, cx: &mut Context<Self>) {
        let board = self.board.clone();
        let panel =
            cx.new(|cx| FeedPanelView::new_expanded_panel(board, index, self.wallpaper.style, cx));
        let close = cx.subscribe(&panel, |view, _, _: &FeedDetailsClosed, cx| {
            view.close_feed_details(cx);
        });
        self.details = Some(FeedDetailsSession {
            index,
            panel,
            _close: close,
        });
        cx.notify();
    }

    fn close_feed_details(&mut self, cx: &mut Context<Self>) {
        self.details = None;
        cx.notify();
    }

    fn toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.is_fullscreen() {
            self.power
                .set_display_mode(false, window, cx.background_executor());
        }
        window.toggle_fullscreen();
        cx.notify();
    }

    fn exit_fullscreen(&mut self, _: &ExitFullscreen, window: &mut Window, cx: &mut Context<Self>) {
        if self.preview.is_some() {
            self.cancel_preview(window, cx);
        } else if self.picker.is_some() {
            self.close_picker(window, cx);
        } else if self.details.is_some() {
            self.close_feed_details(cx);
        } else if window.is_fullscreen() {
            self.toggle_fullscreen(&ToggleFullscreen, window, cx);
        }
    }

    fn quit(&mut self, _: &QuitStatusDisplay, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn refresh(&mut self, _: &RefreshStatus, _: &mut Window, cx: &mut Context<Self>) {
        self.board
            .update(cx, |board, cx| board.refresh_all_feeds(cx));
    }

    fn next_display(&mut self, _: &NextStatusDisplay, _: &mut Window, cx: &mut Context<Self>) {
        self.controller
            .update(cx, |controller, cx| controller.cycle_status_display(cx))
            .ok();
    }
}

impl Render for StatusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.first_frame_requested {
            // GPUI runs next-frame callbacks at the start of the frame after
            // this one, so the logged time is at most one refresh late.
            self.first_frame_requested = true;
            log_startup_milestone(StartupMilestone::FirstViewBuilt);
            window
                .on_next_frame(|_, _| log_startup_milestone(StartupMilestone::FirstFramePresented));
        }
        self.update_display_power(window, cx);
        self.update_slideshow(cx);
        self.update_wallpaper(window, cx);
        let layout = StatusLayout::from_status_window(window);
        let scale = move |value: f32| layout.scale(value);
        let s: Scale = &scale;
        let now = OffsetDateTime::now_utc();
        let offset = self.settings.local_offset;
        self.redraw.redraw_after(status_clock_text_valid_for(now));

        let style = self.wallpaper.style;
        let theme = StatusTheme(style.palette);
        let mut alerts: Vec<AnyElement> = Vec::new();
        if self.preview.is_none() && self.carousel.read(cx).playback_art_unavailable() {
            alerts.push(
                alert_row(
                    s,
                    theme,
                    theme.warn(),
                    "Wallpaper unavailable".into(),
                    "Open Wallpaper to check the image or choose another.".into(),
                )
                .into_any_element(),
            );
        }
        let board = self.board.read(cx);
        for feed in board.status_feeds() {
            if let Some(change) = board.next_freshness_change(feed) {
                self.redraw.redraw_after(change);
            }
            if let Some(alert) =
                feed_alert(s, theme, feed, board.feed_freshness(feed), &self.redraw)
            {
                alerts.push(alert.into_any_element());
            }
        }

        let clock_end = matches!(
            style.arrangement,
            StyleArrangement::Sides { left, .. } if left.clock.is_none()
        );
        let clock = status_clock(s, style, now, offset, clock_end);

        let is_fullscreen = window.is_fullscreen();
        let controls_left = self
            .pointer_moved_at
            .and_then(|at| CONTROLS_VISIBLE_DURATION.checked_sub(at.elapsed()));
        let modal = self.picker.is_some();
        let show_window_controls = !modal && self.details.is_none();
        if show_window_controls && let Some(left) = controls_left {
            self.redraw.redraw_after(left);
        }
        let controls = div()
            .flex()
            .flex_wrap()
            .justify_end()
            .items_center()
            .gap(s(8.))
            .child(
                div()
                    .text_size(s(13.))
                    .text_color(theme.muted())
                    .px(s(8.))
                    .py(s(4.))
                    .rounded(s(6.))
                    .bg(theme.panel())
                    .child(self.settings.display_label.clone()),
            )
            .child(control_button(
                s,
                theme,
                "control-fullscreen",
                if is_fullscreen {
                    "Windowed"
                } else {
                    "Fullscreen"
                },
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle_fullscreen(&ToggleFullscreen, window, cx)
                }),
            ))
            .child(control_button(
                s,
                theme,
                "control-next-display",
                "Next display",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.next_display(&NextStatusDisplay, window, cx)
                }),
            ))
            .child(control_button(
                s,
                theme,
                "control-refresh",
                "Refresh",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.refresh(&RefreshStatus, window, cx)
                }),
            ))
            .child(control_button(
                s,
                theme,
                "control-quit",
                "Quit",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.quit(&QuitStatusDisplay, window, cx)
                }),
            ));
        let notices = div()
            .w_full()
            .flex()
            .flex_col()
            .when(clock_end, |column| column.items_end())
            .gap(s(8.))
            .children(alerts);

        // Each panel is a cached view laid out at its style. GPUI reuses
        // its last drawing until the panel is notified.
        let content = match style.arrangement {
            StyleArrangement::Band => {
                self.band_content(s, layout, clock, notices.w(relative(0.5)), cx)
            }
            StyleArrangement::Sides { left, right } => {
                self.sides_content(s, layout, clock.child(notices), [left, right], cx)
            }
        };
        let shades = match style.arrangement {
            StyleArrangement::Band => Vec::new(),
            StyleArrangement::Sides { left, right } => [(left, false), (right, true)]
                .into_iter()
                .filter(|(area, _)| area.width > 0.)
                .map(|(area, is_right)| area_shade(s, theme, layout, area, is_right))
                .collect(),
        };

        let root = div()
            .id("status-root")
            .when(self.picker.is_none() || self.preview.is_some(), |root| {
                root.key_context(STATUS_VIEW_KEY_CONTEXT)
            })
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::exit_fullscreen))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::next_display))
            .capture_any_mouse_down(cx.listener(|this, event: &gpui::MouseDownEvent, _, _| {
                if event.button == MouseButton::Left {
                    this.controls_latched = this
                        .pointer_moved_at
                        .is_some_and(|at| at.elapsed() < CONTROLS_VISIBLE_DURATION);
                    this.pointer_pressed = true;
                }
            }))
            .capture_any_mouse_up(cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                if event.button == MouseButton::Left && this.pointer_pressed {
                    this.pointer_pressed = false;
                    this.pointer_moved_at = Some(Instant::now());
                    cx.notify();
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.pointer_pressed {
                        this.pointer_pressed = false;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, _, _, cx| {
                // Draw again only to show hidden controls. The redraw timer
                // hides them after the last move.
                let hidden = this
                    .pointer_moved_at
                    .is_none_or(|at| at.elapsed() >= CONTROLS_VISIBLE_DURATION);
                this.pointer_moved_at = Some(Instant::now());
                if hidden {
                    cx.notify();
                }
            }))
            .relative()
            .size_full()
            .bg(theme.background())
            .font_family(STATUS_MONO_FONT_FAMILY)
            .line_height(relative(1.25))
            .text_color(theme.text())
            .children(self.wallpaper.image.clone().map(|image| {
                img(image)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
            }))
            .child(div().absolute().inset_0().bg(theme.scrim()))
            .children(shades)
            .child(content)
            .when(!modal, |root| {
                root.child(
                    div()
                        .absolute()
                        .top(s(8.))
                        .right(s(8.))
                        .left(s(8.))
                        .flex()
                        .justify_end()
                        .items_center()
                        .gap(s(8.))
                        .when(
                            show_window_controls
                                && if self.pointer_pressed {
                                    self.controls_latched
                                } else {
                                    controls_left.is_some()
                                },
                            |bar| bar.child(controls),
                        )
                        .child(
                            control_button(
                                s,
                                theme,
                                "open-wallpaper-picker",
                                "Wallpaper",
                                cx.listener(|view, _, window, cx| view.open_picker(window, cx)),
                            )
                            .min_h(px(44.))
                            .flex_none()
                            .flex()
                            .items_center(),
                        ),
                )
            })
            .children(
                self.picker
                    .as_ref()
                    .and_then(|picker| self.preview.is_none().then(|| picker.view.clone())),
            )
            .children(
                self.preview
                    .as_ref()
                    .map(|preview| self.wallpaper_preview_bar(s, theme, preview, cx)),
            );
        self.redraw.schedule_status_redraw(cx);
        root
    }
}
