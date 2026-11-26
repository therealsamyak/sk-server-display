//! Status view: the full-screen GPUI layout for the status display.
//!
//! The root view draws the wallpaper, the clock, the alerts, and the window
//! controls. Each feed panel is a cached child view (see `status_panel`), so
//! a root redraw for the clock reuses the panels as they are.

mod status_slideshow_swipe;
mod status_update_view;
mod status_view_layout;
mod status_wallpaper_view;

use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, HoverListenerMode, IntoElement,
    KeyBinding, MouseButton, Render, Subscription, WeakEntity, Window, actions, div, prelude::*,
    relative,
};
use time::{OffsetDateTime, UtcOffset};

use crate::{
    metrics::{StatusBoard, StatusFeed},
    platform::{StatusPower, status_update::finish_app_update},
    status_fonts::STATUS_MONO_FONT_FAMILY,
    status_startup::{StartupMilestone, log_startup_milestone},
    wallpaper::StatusCarousel,
};

use super::{
    status_btop::{BtopWindowClosed, BtopWindowView},
    status_format::status_clock_text_valid_for,
    status_panel::{FeedBtopRequested, FeedDetailsClosed, FeedPanelView},
    status_redraw::StatusRedrawTimer,
    status_theme::{Scale, StatusLayout, StatusTheme},
    status_window::StatusDisplayApp,
};
use status_slideshow_swipe::{IncomingPanels, SlideshowSwipe};
use status_update_view::StatusUpdateDialog;
use status_view_layout::{SCREEN_PADDING, control_button};
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
        /// Reloads the active configuration and polls its feeds.
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
    btop: Option<BtopSession>,
    pointer_pressed: bool,
    swipe: Option<SlideshowSwipe>,
    incoming_panels: Option<IncomingPanels>,
    /// A swipe moved the pointer since the last press, so its release is not a tap.
    swipe_consumed: bool,
    controls_latched: bool,
    toolbar_hovered: bool,
    power: StatusPower,
    pointer_moved_at: Option<Instant>,
    first_frame_requested: bool,
    refresh_failed: bool,
    update_busy: bool,
    update_dialog: Option<StatusUpdateDialog>,
    redraw: StatusRedrawTimer,
    _subscriptions: Vec<Subscription>,
}

struct FeedDetailsSession {
    index: usize,
    panel: Entity<FeedPanelView>,
    _close: Subscription,
    _btop: Subscription,
}

struct BtopSession {
    view: Entity<BtopWindowView>,
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
                        view.swipe = None;
                    }
                    cx.notify();
                }) {
                    log::debug!("status window closed: {error:#}");
                }
            }),
            cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() {
                    view.pointer_pressed = false;
                    view.swipe = None;
                }
                cx.notify();
            }),
        ];
        let mut view = Self {
            board,
            carousel,
            controller,
            settings,
            focus_handle,
            panels: Vec::new(),
            host_rows: 0,
            carded: Vec::new(),
            wallpaper: StatusWallpaper::default(),
            picker: None,
            preview: None,
            preview_error: None,
            details: None,
            btop: None,
            pointer_pressed: false,
            swipe: None,
            incoming_panels: None,
            swipe_consumed: false,
            controls_latched: false,
            toolbar_hovered: false,
            power: StatusPower::default(),
            pointer_moved_at: None,
            first_frame_requested: false,
            refresh_failed: false,
            update_busy: false,
            update_dialog: None,
            redraw: StatusRedrawTimer::default(),
            _subscriptions: subscriptions,
        };
        view.rebuild_feed_panels(cx);
        view
    }

    fn rebuild_feed_panels(&mut self, cx: &mut Context<Self>) {
        let feeds = self.board.read(cx).status_feeds();
        let is_host = |feed: &&StatusFeed| feed.spec.collector.is_host_collector();
        let host_count = feeds.iter().filter(is_host).count();
        // Up to two hosts get one column each. More hosts share one column.
        self.host_rows = if host_count <= 2 { 1 } else { host_count };
        // Devices with a host card do not repeat in the tailnet list.
        self.carded = feeds
            .iter()
            .filter_map(|feed| feed.spec.collector.tailnet_device_name().map(str::to_owned))
            .collect();
        self.panels = (0..feeds.len())
            .map(|index| {
                cx.new(|cx| {
                    let mut panel = FeedPanelView::new_feed_panel(
                        self.board.clone(),
                        index,
                        self.carded.clone(),
                        cx,
                    );
                    panel.set_panel_style(self.wallpaper.style, cx);
                    panel
                })
            })
            .collect();
    }

    fn update_display_power(&mut self, window: &Window, cx: &Context<Self>) {
        self.power.set_display_mode(
            window.is_fullscreen() && window.is_visible(),
            window,
            cx.background_executor(),
        );
    }

    fn open_feed_details(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let board = self.board.clone();
        let panel =
            cx.new(|cx| FeedPanelView::new_expanded_panel(board, index, self.wallpaper.style, cx));
        let close = cx.subscribe(&panel, |view, _, _: &FeedDetailsClosed, cx| {
            view.close_feed_details(cx);
        });
        let btop = cx.subscribe_in(
            &panel,
            window,
            move |view, _, _: &FeedBtopRequested, window, cx| {
                view.open_btop_window(index, window, cx);
            },
        );
        self.details = Some(FeedDetailsSession {
            index,
            panel,
            _close: close,
            _btop: btop,
        });
        cx.notify();
    }

    /// Opens the btop window of a host, or brings an open one to that host.
    fn open_btop_window(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .btop
            .as_ref()
            .is_some_and(|btop| btop.view.read(cx).btop_feed_index() == index)
        {
            return;
        }
        let board = self.board.clone();
        let style = self.wallpaper.style;
        let view = cx.new(|cx| BtopWindowView::new_btop_window(board, index, style, window, cx));
        let close = cx.subscribe(&view, |view, _, _: &BtopWindowClosed, cx| {
            view.btop = None;
            cx.notify();
        });
        self.btop = Some(BtopSession {
            view,
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
        if self.update_dialog.is_some() {
            self.close_update_dialog(cx);
        } else if self.preview.is_some() {
            self.cancel_preview(window, cx);
        } else if self.picker.is_some() {
            self.close_picker(window, cx);
        } else if self.btop.is_some() {
            self.btop = None;
            cx.notify();
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
        if let Err(error) = self
            .controller
            .update(cx, |controller, cx| controller.refresh_status_profile(cx))
        {
            log::debug!("status display closed before refresh: {error:#}");
        }
    }

    /// Apply a validated profile to the running dashboard.
    pub fn apply_status_profile(
        &mut self,
        board: Option<Entity<StatusBoard>>,
        presets_changed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(board) = board {
            self.board = board;
            self.details = None;
            self.btop = None;
            self.incoming_panels = None;
            self.rebuild_feed_panels(cx);
        }
        if presets_changed {
            self.swipe = None;
            self.incoming_panels = None;
            self.reload_preset_wallpapers(window, cx);
        }
        self.refresh_failed = false;
        cx.notify();
    }

    /// Indicate that refreshing the configuration failed.
    pub fn show_refresh_error(&mut self, cx: &mut Context<Self>) {
        self.refresh_failed = true;
        cx.notify();
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
            let view = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                log_startup_milestone(StartupMilestone::FirstFramePresented);
                if let Some(error) = finish_app_update() {
                    let _ = view.update(cx, |view, cx| {
                        view.update_dialog = Some(StatusUpdateDialog::Notice {
                            title: "Update failed",
                            detail: Some(error),
                        });
                        cx.notify();
                    });
                }
            });
        }
        self.update_display_power(window, cx);
        self.finish_swipe(window, cx);
        self.update_wallpaper(window, cx);
        self.warm_slideshow(window, cx);
        self.update_slideshow(window, cx);
        self.prepare_incoming_panels(window, cx);
        let layout = StatusLayout::from_status_window(window);
        let scale = move |value: f32| layout.scale(value);
        let s: Scale = &scale;
        let now = OffsetDateTime::now_utc();
        self.redraw.redraw_after(status_clock_text_valid_for(now));

        let style = self.wallpaper.style;
        let theme = StatusTheme(style.palette);
        let is_fullscreen = window.is_fullscreen();
        let controls_left = self
            .pointer_moved_at
            .and_then(|at| CONTROLS_VISIBLE_DURATION.checked_sub(at.elapsed()));
        let modal = self.picker.is_some() || self.update_dialog.is_some();
        let toolbar_present = !modal || self.preview.is_some();
        if !toolbar_present {
            self.toolbar_hovered = false;
        }
        let show_window_controls =
            !modal && self.details.is_none() && !self.automatic_slide_in_motion();
        let controls_visible = if self.pointer_pressed || self.swipe_in_motion() {
            self.controls_latched
        } else {
            self.toolbar_hovered || controls_left.is_some()
        };
        let slideshow_taps = !modal
            && self.preview.is_none()
            && self.carousel.read(cx).state.playing_playlist().is_some();
        if show_window_controls
            && !self.toolbar_hovered
            && let Some(left) = controls_left
        {
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
            .child(
                control_button(
                    s,
                    theme,
                    "control-update",
                    concat!("v", env!("CARGO_PKG_VERSION")),
                    cx.listener(|this, _, window, cx| this.check_for_updates(window, cx)),
                )
                .aria_label(concat!(
                    "Check for updates. Version ",
                    env!("CARGO_PKG_VERSION")
                )),
            )
            .child(control_button(
                s,
                theme,
                "control-quit",
                "Quit",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.quit(&QuitStatusDisplay, window, cx)
                }),
            ));
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
            .capture_any_mouse_down(cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                if event.button == MouseButton::Left {
                    this.begin_swipe(event.position, cx);
                    this.controls_latched = this.toolbar_hovered
                        || this
                            .pointer_moved_at
                            .is_some_and(|at| at.elapsed() < CONTROLS_VISIBLE_DURATION);
                    this.pointer_pressed = true;
                }
            }))
            .capture_any_mouse_up(cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                if event.button == MouseButton::Left {
                    this.end_swipe(window, cx);
                }
                if event.button == MouseButton::Left && this.pointer_pressed {
                    this.pointer_pressed = false;
                    if !this.swipe_consumed {
                        this.pointer_moved_at = Some(Instant::now());
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.end_swipe(window, cx);
                    if this.pointer_pressed {
                        this.pointer_pressed = false;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                    if event.dragging() {
                        this.drag_swipe(event.position, window, cx);
                    }
                    if this.swipe_in_motion() {
                        return;
                    }
                    // Draw again only to show hidden controls. The redraw timer
                    // hides them after the last move.
                    let hidden = this
                        .pointer_moved_at
                        .is_none_or(|at| at.elapsed() >= CONTROLS_VISIBLE_DURATION);
                    this.pointer_moved_at = Some(Instant::now());
                    if hidden {
                        cx.notify();
                    }
                }),
            )
            .relative()
            .size_full()
            .bg(theme.background())
            .font_family(STATUS_MONO_FONT_FAMILY)
            .line_height(relative(1.25))
            .text_color(theme.text())
            .child(self.slideshow_pages(window, layout, now, cx))
            .when(slideshow_taps, |root| {
                root.child(self.slideshow_edge(s, false, cx))
                    .child(self.slideshow_edge(s, true, cx))
            })
            .when(toolbar_present, |root| {
                root.child(
                    div()
                        .absolute()
                        .top(s(SCREEN_PADDING))
                        .right(s(SCREEN_PADDING))
                        .left(s(SCREEN_PADDING))
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .id("status-toolbar")
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(s(8.))
                                .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
                                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                    this.toolbar_hovered = *hovered;
                                    if !hovered {
                                        this.pointer_moved_at = Some(Instant::now());
                                    }
                                    cx.notify();
                                }))
                                .when(
                                    self.preview.is_some()
                                        || (show_window_controls && controls_visible),
                                    |bar| bar.child(controls),
                                )
                                .child(
                                    control_button(
                                        s,
                                        theme,
                                        "open-wallpaper-picker",
                                        "Wallpaper",
                                        cx.listener(|view, _, window, cx| {
                                            view.open_picker(window, cx)
                                        }),
                                    )
                                    .flex_none(),
                                ),
                        ),
                )
            })
            .children(
                self.btop
                    .as_ref()
                    .filter(|_| !modal && self.preview.is_none())
                    .map(|btop| btop.view.clone()),
            )
            .children(
                self.picker
                    .as_ref()
                    .and_then(|picker| self.preview.is_none().then(|| picker.view.clone())),
            )
            .children(
                self.preview
                    .as_ref()
                    .map(|preview| self.wallpaper_preview_bar(s, theme, preview, cx)),
            )
            .children(
                self.update_dialog
                    .as_ref()
                    .map(|dialog| self.render_update_dialog(dialog, cx)),
            );
        self.redraw.schedule_status_redraw(cx);
        root
    }
}
