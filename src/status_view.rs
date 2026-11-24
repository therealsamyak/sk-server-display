//! Status view: the full-screen GPUI layout for the status display.
//!
//! The root view draws the wallpaper, the clock, the alerts, and the window
//! controls. Each feed panel is a cached child view (see `status_panel`), so
//! a root redraw for the clock reuses the panels as they are.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    status_board::StatusBoard,
    status_collector::StatusCollector,
    status_feed::StatusFeed,
    status_fonts::STATUS_MONO_FONT_FAMILY,
    status_format::{
        format_status_age, format_status_clock, format_status_date, status_clock_text_valid_for,
    },
    status_freshness::FeedFreshness,
    status_panel::{FeedPanelView, HostBodyKind, side_panel_height},
    status_redraw::StatusRedrawTimer,
    status_startup::{StartupMilestone, log_startup_milestone},
    status_style::{PanelGroup, StyleAnchor, StyleArea, StyleArrangement, active_status_style},
    status_theme::{PANEL_GAP, PRESENCE_PANEL_HEIGHT, Scale, StatusLayout, StatusTheme},
    status_wallpaper::load_status_wallpaper,
    status_window::StatusDisplayApp,
};
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, FontWeight,
    Hsla, IntoElement, ObjectFit, Render, RenderImage, Size, Stateful, StyleRefinement,
    StyledImage as _, Subscription, Task, WeakEntity, Window, actions, div, img, linear_color_stop,
    linear_gradient, prelude::*, relative,
};
use time::{OffsetDateTime, UtcOffset};

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
pub const STATUS_VIEW_KEY_CONTEXT: &str = "StatusDisplay";

/// Time that the window controls stay visible after the pointer moves.
const CONTROLS_VISIBLE_DURATION: Duration = Duration::from_secs(3);

/// Per-window settings of the status view.
pub struct StatusViewSettings {
    /// Display description, such as `Display 1 of 2 (primary)`.
    pub display_label: String,
    pub local_offset: UtcOffset,
    /// Background image file. `None` gives a solid background.
    pub wallpaper: Option<PathBuf>,
}

/// Status view: clock, alerts, host panels, services, disks, and tailnet.
pub struct StatusView {
    board: Entity<StatusBoard>,
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
    pointer_moved_at: Option<Instant>,
    first_frame_requested: bool,
    redraw: StatusRedrawTimer,
    _subscriptions: Vec<Subscription>,
}

/// Wallpaper of one window: the scaled image and the size it covers.
#[derive(Default)]
struct StatusWallpaper {
    image: Option<Arc<RenderImage>>,
    /// Size in physical pixels of the last load request.
    requested: Size<u32>,
    task: Option<Task<()>>,
}

impl StatusView {
    /// Creates the status view for one window.
    pub fn new_status_view(
        settings: StatusViewSettings,
        board: Entity<StatusBoard>,
        controller: WeakEntity<StatusDisplayApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        // The board emits events and does not notify, so the root view draws
        // again only for its own changes and for panel changes.
        let subscriptions = vec![cx.observe_window_activation(window, |_, _, cx| cx.notify())];
        let feeds = board.read(cx).status_feeds();
        let is_host = |feed: &&StatusFeed| feed.spec.collector.is_host_collector();
        let host_count = feeds.iter().filter(is_host).count();
        // Up to two hosts get one column each. More hosts share one column.
        let host_rows = if host_count <= 2 { 1 } else { host_count };
        // Devices with a host card do not repeat in the tailnet list.
        let carded: Vec<String> = feeds
            .iter()
            .filter(is_host)
            .map(|feed| feed.spec.title.clone())
            .collect();
        let host_body = match active_status_style().arrangement {
            StyleArrangement::Sides { .. } => HostBodyKind::Stacked,
            StyleArrangement::Band if host_rows > 1 => HostBodyKind::Row,
            StyleArrangement::Band => HostBodyKind::Full,
        };
        let panels = (0..feeds.len())
            .map(|index| {
                let board = board.clone();
                let carded = carded.clone();
                cx.new(|cx| {
                    FeedPanelView::new_feed_panel(board, index, host_rows, host_body, carded, cx)
                })
            })
            .collect();
        Self {
            board,
            controller,
            settings,
            focus_handle,
            panels,
            host_rows,
            carded,
            wallpaper: StatusWallpaper::default(),
            pointer_moved_at: None,
            first_frame_requested: false,
            redraw: StatusRedrawTimer::default(),
            _subscriptions: subscriptions,
        }
    }

    /// Loads the wallpaper at the size of the window's display, once. A
    /// move to a larger display loads it again at the new size.
    fn load_wallpaper_for_window(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(path) = self.settings.wallpaper.clone() else {
            return;
        };
        let logical = window
            .display(cx)
            .map(|display| display.bounds().size)
            .unwrap_or_else(|| window.viewport_size());
        let scale = window.scale_factor();
        let target = Size::new(
            (f32::from(logical.width) * scale).ceil() as u32,
            (f32::from(logical.height) * scale).ceil() as u32,
        );
        let requested = self.wallpaper.requested;
        if target.width <= requested.width && target.height <= requested.height {
            return;
        }
        self.wallpaper.requested = Size::new(
            target.width.max(requested.width),
            target.height.max(requested.height),
        );
        let target = self.wallpaper.requested;
        let load = cx.background_spawn(async move { load_status_wallpaper(&path, target) });
        self.wallpaper.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = load.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(image) => {
                    if let Some(old) = this.wallpaper.image.replace(image) {
                        window.drop_image(old).ok();
                    }
                    cx.notify();
                }
                Err(error) => log::error!("{error:#}"),
            })
            .ok();
        }));
    }

    fn toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
    }

    fn exit_fullscreen(&mut self, _: &ExitFullscreen, window: &mut Window, _: &mut Context<Self>) {
        if window.is_fullscreen() {
            window.toggle_fullscreen();
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
        self.load_wallpaper_for_window(window, cx);
        let layout = StatusLayout::from_status_window(window);
        let scale = move |value: f32| layout.scale(value);
        let s: Scale = &scale;
        let now = OffsetDateTime::now_utc();
        let offset = self.settings.local_offset;
        self.redraw.redraw_after(status_clock_text_valid_for(now));

        let mut alerts: Vec<AnyElement> = Vec::new();
        if window.is_window_active() {
            alerts.push(
                alert_row(
                    s,
                    StatusTheme::warn(),
                    "This window has keyboard focus.".into(),
                    "Moonlight input goes here, not to the game. Click the game to return it."
                        .into(),
                )
                .into_any_element(),
            );
        }
        let board = self.board.read(cx);
        for feed in board.status_feeds() {
            if let Some(change) = board.next_freshness_change(feed) {
                self.redraw.redraw_after(change);
            }
            if let Some(alert) = feed_alert(s, feed, board.feed_freshness(feed), &self.redraw) {
                alerts.push(alert.into_any_element());
            }
        }

        let style = active_status_style();
        let clock_end = matches!(
            style.arrangement,
            StyleArrangement::Sides { left, .. } if left.clock.is_none()
        );
        let clock = status_clock(s, now, offset, clock_end);

        let is_fullscreen = window.is_fullscreen();
        let controls_left = self
            .pointer_moved_at
            .and_then(|at| CONTROLS_VISIBLE_DURATION.checked_sub(at.elapsed()));
        if let Some(left) = controls_left {
            self.redraw.redraw_after(left);
        }
        let controls = div()
            .flex()
            .flex_wrap()
            .when(clock_end, |row| row.justify_end())
            .items_center()
            .gap(s(8.))
            .child(
                div()
                    .pr(s(8.))
                    .text_size(s(13.))
                    .text_color(StatusTheme::muted())
                    .child(self.settings.display_label.clone()),
            )
            .child(control_button(
                s,
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
                "control-next-display",
                "Next display",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.next_display(&NextStatusDisplay, window, cx)
                }),
            ))
            .child(control_button(
                s,
                "control-refresh",
                "Refresh",
                cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.refresh(&RefreshStatus, window, cx)
                }),
            ))
            .child(control_button(
                s,
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
            .when(controls_left.is_some(), |column| column.child(controls))
            .children(alerts);

        // Each panel is a cached view laid out at its style. GPUI reuses
        // its last drawing until the panel is notified.
        let content = match style.arrangement {
            StyleArrangement::Band => {
                self.band_content(s, layout, board, clock, notices.w(relative(0.5)))
            }
            StyleArrangement::Sides { left, right } => {
                self.sides_content(s, layout, board, clock.child(notices), left, right)
            }
        };
        let shades = match style.arrangement {
            StyleArrangement::Band => Vec::new(),
            StyleArrangement::Sides { left, right } => [(left, false), (right, true)]
                .into_iter()
                .filter(|(area, _)| area.width > 0.)
                .map(|(area, is_right)| area_shade(s, layout, area, is_right))
                .collect(),
        };

        let root = div()
            .id("status-root")
            .key_context(STATUS_VIEW_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::exit_fullscreen))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::next_display))
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
            .bg(StatusTheme::background())
            .font_family(STATUS_MONO_FONT_FAMILY)
            .line_height(relative(1.25))
            .text_color(StatusTheme::text())
            .children(self.wallpaper.image.clone().map(|image| {
                img(image)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
            }))
            .child(div().absolute().inset_0().bg(StatusTheme::scrim()))
            .children(shades)
            .child(content);
        self.redraw.schedule_status_redraw(cx);
        root
    }
}

/// Alert for a feed that is stale or offline. Live feeds give none.
fn feed_alert(
    s: Scale,
    feed: &StatusFeed,
    freshness: FeedFreshness,
    redraw: &StatusRedrawTimer,
) -> Option<Div> {
    let name = format!("{} ({})", feed.spec.title, feed.spec.source);
    match freshness {
        FeedFreshness::Stale => Some(alert_row(
            s,
            StatusTheme::warn(),
            format!("{name} is not responding."),
            format!(
                "Data is {} old. {}",
                feed.last_success_age()
                    .map(|age| redraw.status_age_text(age))
                    .unwrap_or_default(),
                feed.last_poll_error().unwrap_or_default(),
            ),
        )),
        FeedFreshness::Offline => Some(alert_row(
            s,
            StatusTheme::bad(),
            format!("{name} is offline."),
            format!(
                "Retrying every {}.",
                format_status_age(feed.policy.poll_interval),
            ),
        )),
        FeedFreshness::Waiting | FeedFreshness::Live => None,
    }
}

fn alert_row(s: Scale, color: Hsla, title: String, detail: String) -> Div {
    div()
        .max_w_full()
        .flex()
        .flex_col()
        .gap(s(2.))
        .px(s(16.))
        .py(s(8.))
        .rounded(s(8.))
        .bg(StatusTheme::panel())
        .border_1()
        .border_color(color.opacity(0.7))
        .child(
            div()
                .text_size(s(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(title),
        )
        .child(
            div()
                .text_size(s(13.))
                .text_color(StatusTheme::muted())
                .text_ellipsis()
                .line_clamp(2)
                .child(detail),
        )
}

fn control_button(
    s: Scale,
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .px(s(12.))
        .py(s(4.))
        .rounded(s(6.))
        .bg(StatusTheme::panel())
        .border_1()
        .border_color(StatusTheme::panel_border())
        .text_size(s(13.))
        .text_color(StatusTheme::muted())
        .cursor_pointer()
        .hover(|style| style.text_color(StatusTheme::text()))
        .on_click(on_click)
        .child(label)
}

impl StatusView {
    /// Band arrangement: the clock and the notices at the top, all panels in
    /// a band at the bottom.
    fn band_content(
        &self,
        s: Scale,
        layout: StatusLayout,
        board: &StatusBoard,
        clock: Div,
        notices: Div,
    ) -> Div {
        let top = div()
            .flex_1()
            .min_h_0()
            .flex()
            .justify_between()
            .items_start()
            .gap(s(32.))
            .child(clock)
            .child(notices.items_end());
        let panel_style = || StyleRefinement::default().flex_1().min_w_0().min_h_0();
        // A status card has a fixed height. Metric cards share the rest of
        // the column.
        let status_card_style = || {
            StyleRefinement::default()
                .flex_none()
                .min_w_0()
                .h(s(PRESENCE_PANEL_HEIGHT))
        };
        let mut host_panels = Vec::new();
        let mut service_panels = Vec::new();
        let mut side_panels = Vec::new();
        for (feed, panel) in board.status_feeds().iter().zip(&self.panels) {
            let style = match feed.spec.collector {
                StatusCollector::TailnetHost(_) => status_card_style(),
                _ => panel_style(),
            };
            let panel = panel.clone().cached(style).into_any_element();
            match &feed.spec.collector {
                StatusCollector::Glances(_)
                | StatusCollector::Local(_)
                | StatusCollector::TailnetHost(_) => host_panels.push(panel),
                StatusCollector::Kuma(_) => service_panels.push(panel),
                StatusCollector::Scrutiny(_) | StatusCollector::Tailscale(_) => {
                    side_panels.push(panel)
                }
            }
        }
        let host_count = host_panels.len();
        let column = |panels: Vec<AnyElement>, grow: f32| {
            (!panels.is_empty()).then(|| {
                let mut column = div()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap(s(PANEL_GAP))
                    .children(panels);
                column.style().flex_grow = Some(grow);
                column.style().flex_basis = Some(relative(0.).into());
                column
            })
        };
        let host_columns = if self.host_rows == 1 {
            let grow = if host_count == 1 { 1.3 } else { 1. };
            host_panels
                .into_iter()
                .filter_map(|panel| column(vec![panel], grow))
                .collect()
        } else {
            column(host_panels, 1.3).into_iter().collect::<Vec<_>>()
        };
        let panels = div()
            .h(s(layout.panel_area))
            .flex()
            .gap(s(PANEL_GAP))
            .children(host_columns)
            .children(column(service_panels, 1.))
            .children(column(side_panels, 1.));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .gap(s(24.))
            .p(s(SCREEN_PADDING))
            .child(top)
            .child(panels)
    }

    /// Side arrangement: one column area at each edge, the middle free for
    /// the art. `clock` holds the clock and the notices.
    fn sides_content(
        &self,
        s: Scale,
        layout: StatusLayout,
        board: &StatusBoard,
        clock: Div,
        left: StyleArea,
        right: StyleArea,
    ) -> Div {
        let mut hosts = Vec::new();
        let mut others = Vec::new();
        for (feed, panel) in board.status_feeds().iter().zip(&self.panels) {
            let height = side_panel_height(feed, &self.carded);
            // Only a host card with metrics gives up height when the column
            // is full: its CPU graph is shorter then.
            let style = match feed.spec.collector {
                StatusCollector::Glances(_) | StatusCollector::Local(_) => {
                    StyleRefinement::default()
                        .flex_shrink(1.)
                        .min_h(s(STACKED_HOST_MIN_HEIGHT))
                }
                _ => StyleRefinement::default().flex_none(),
            }
            .w_full()
            .h(s(height));
            let panel = panel.clone().cached(style).into_any_element();
            if feed.spec.collector.is_host_collector() {
                hosts.push(panel);
            } else {
                others.push(panel);
            }
        }
        let mut clock = Some(clock);
        let mut groups = Some((hosts, others));
        let inner_width = layout.width - 2. * SCREEN_PADDING;
        let mut area = |area: StyleArea, is_right: bool| {
            (area.width > 0.).then(|| {
                let area_clock = area
                    .clock
                    .and_then(|anchor| clock.take().map(|c| (anchor, c)));
                let (hosts, others) = groups.take().unwrap_or_default();
                let (mut hosts, mut others) = (Some(hosts), Some(others));
                let mut columns: Vec<SideColumn> = area
                    .groups
                    .iter()
                    .map(|group| {
                        let panels = match group {
                            PanelGroup::Hosts => hosts.take(),
                            PanelGroup::Others => others.take(),
                        };
                        let grow = match group {
                            PanelGroup::Hosts => 1.,
                            PanelGroup::Others => OTHERS_COLUMN_GROW,
                        };
                        (None, panels.unwrap_or_default(), grow)
                    })
                    .collect();
                // Panels not placed in this area go to the other area.
                groups = Some((hosts.unwrap_or_default(), others.unwrap_or_default()));
                // The clock stands on the host column, or alone.
                let clock_column = area
                    .groups
                    .iter()
                    .position(|group| *group == PanelGroup::Hosts)
                    .unwrap_or(0);
                match columns.get_mut(clock_column) {
                    Some(column) => column.0 = area_clock,
                    None => columns.push((area_clock, Vec::new(), 1.)),
                }
                if is_right {
                    columns.reverse();
                }
                div()
                    .flex_none()
                    .w(s(area.width * inner_width))
                    .h_full()
                    .flex()
                    .gap(s(PANEL_GAP))
                    .children(columns.into_iter().map(|(clock, panels, grow)| {
                        side_column(s, clock, panels, area.panels, grow)
                    }))
            })
        };
        // The left area takes its groups first. The right area gets the rest.
        let left_area = area(left, false);
        let right_area = area(right, true);
        div()
            .relative()
            .size_full()
            .flex()
            .gap(s(PANEL_GAP))
            .p(s(SCREEN_PADDING))
            .children(left_area)
            .child(div().flex_1())
            .children(right_area)
    }
}

/// One column of a side area: the clock block with its anchor, if any, and
/// the panels.
type SideColumn = (Option<(StyleAnchor, Div)>, Vec<AnyElement>, f32);

/// Width share of the services, disks, and tailnet column next to a host
/// column. Its rows have more columns than the host cards.
const OTHERS_COLUMN_GROW: f32 = 1.15;

/// Lowest height of a stacked host card, in design pixels.
const STACKED_HOST_MIN_HEIGHT: f32 = 152.;

/// Padding at the window edges, in design pixels.
const SCREEN_PADDING: f32 = 32.;

/// Width of the shade fade beyond a side area, in design pixels.
const SHADE_FADE: f32 = 200.;

/// One column of a side area: the clock block, if any, and a panel stack.
fn side_column(
    s: Scale,
    clock: Option<(StyleAnchor, Div)>,
    panels: Vec<AnyElement>,
    anchor: StyleAnchor,
    grow: f32,
) -> Div {
    let stack = (!panels.is_empty()).then(|| {
        div()
            .w_full()
            .min_h_0()
            .flex_shrink(1.)
            .flex()
            .flex_col()
            .gap(s(PANEL_GAP))
            .children(panels)
    });
    let mut column = div().min_w_0().h_full().flex().flex_col().gap(s(24.));
    column.style().flex_grow = Some(grow);
    column.style().flex_basis = Some(relative(0.).into());
    match (clock, stack) {
        (Some((clock_anchor, clock)), Some(stack)) => {
            let clock = clock.flex_none();
            // A free space between the two blocks puts each at its own edge.
            let spacer = (clock_anchor != anchor).then(|| div().flex_1());
            let column = column.when(
                clock_anchor == StyleAnchor::Bottom && anchor == StyleAnchor::Bottom,
                |column| column.justify_end(),
            );
            match clock_anchor {
                StyleAnchor::Top => column.child(clock).children(spacer).child(stack),
                StyleAnchor::Bottom => column.child(stack).children(spacer).child(clock),
            }
        }
        (Some((clock_anchor, clock)), None) => column
            .when(clock_anchor == StyleAnchor::Bottom, |column| {
                column.justify_end()
            })
            .child(clock),
        (None, Some(stack)) => column
            .when(anchor == StyleAnchor::Bottom, |column| column.justify_end())
            .child(stack),
        (None, None) => column,
    }
}

/// Shade behind one side area. It fades out toward the middle of the screen.
fn area_shade(s: Scale, layout: StatusLayout, area: StyleArea, is_right: bool) -> Div {
    let solid = SCREEN_PADDING + area.width * (layout.width - 2. * SCREEN_PADDING);
    let total = solid + SHADE_FADE;
    let shade = StatusTheme::shade();
    let gradient = linear_gradient(
        if is_right { 270. } else { 90. },
        linear_color_stop(shade, solid / total),
        linear_color_stop(shade.opacity(0.), 1.),
    );
    let strip = div().absolute().top_0().bottom_0().w(s(total)).bg(gradient);
    if is_right {
        strip.right_0()
    } else {
        strip.left_0()
    }
}

/// Clock and date. `at_end` aligns them to the right edge.
fn status_clock(s: Scale, now: OffsetDateTime, offset: UtcOffset, at_end: bool) -> Div {
    let kind = active_status_style().kind;
    div()
        .flex()
        .flex_col()
        .when(at_end, |clock| clock.items_end())
        .gap(s(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .when(at_end, |clock| clock.items_end())
                .font_family(kind.clock_family)
                .child(
                    div()
                        .text_size(s(kind.clock_size))
                        .line_height(relative(1.))
                        .whitespace_nowrap()
                        .when(kind.clock_light, |clock| {
                            clock.font_weight(FontWeight::LIGHT)
                        })
                        .text_color(StatusTheme::clock())
                        .child(format_status_clock(now, offset)),
                )
                .child(
                    div()
                        .px(s(4.))
                        .whitespace_nowrap()
                        .text_size(s(28.))
                        .text_color(StatusTheme::date())
                        .child(format_status_date(now, offset)),
                ),
        )
}
