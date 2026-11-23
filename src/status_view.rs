//! Status view: the full-screen GPUI layout for the status display.

use std::{
    collections::VecDeque,
    path::PathBuf,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, FocusHandle, FontWeight, Hsla, IntoElement,
    ObjectFit, Pixels, Render, Stateful, StyledImage as _, Subscription, WeakEntity, Window,
    actions, div, img, prelude::*, px, relative, rgb,
};
use time::{OffsetDateTime, UtcOffset};

use crate::{
    status_board::{HOST_HISTORY_SAMPLES, HostHistory, StatusBoard, StatusFeed},
    status_collector::StatusCollector,
    status_fonts::{STATUS_CLOCK_FONT_FAMILY, STATUS_PANEL_FONT_FAMILY},
    status_format::{
        format_status_age, format_status_bytes, format_status_clock, format_status_date,
        format_status_rate, format_status_uptime, format_status_usage,
    },
    status_freshness::FeedFreshness,
    status_model::{
        DiskHealth, DiskState, FeedData, HostMetrics, ServiceHealth, ServiceState,
        TailnetConnection, TailnetDevice,
    },
    status_window::StatusDisplayApp,
};

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

/// Design reference width. The layout scales from this width.
const DESIGN_WIDTH: f32 = 1600.0;

/// Space between panels, in design pixels.
const PANEL_GAP: f32 = 16.0;

/// Host panel height, in design pixels, without the core grid: frame, text
/// rows, and the smallest CPU and network graphs.
const HOST_PANEL_BASE_HEIGHT: f32 = 330.0;

/// Height of one core grid row, in design pixels.
const CORE_GRID_ROW_HEIGHT: f32 = 26.0;

/// Part of the window height that the panels use. The wallpaper and the
/// clock fill the rest.
const PANEL_AREA_HEIGHT: f32 = 0.64;

struct StatusTheme;

impl StatusTheme {
    /// Window color without a wallpaper, and while the wallpaper loads.
    fn background() -> Hsla {
        rgb(0x10151c).into()
    }
    /// Dark layer over the wallpaper, so white text stays readable.
    fn scrim() -> Hsla {
        Hsla::black().opacity(0.28)
    }
    fn panel() -> Hsla {
        Hsla::from(rgb(0x0c1016)).opacity(0.78)
    }
    fn panel_border() -> Hsla {
        Hsla::white().opacity(0.10)
    }
    fn track() -> Hsla {
        Hsla::white().opacity(0.10)
    }
    fn text() -> Hsla {
        rgb(0xeef1f4).into()
    }
    fn muted() -> Hsla {
        rgb(0x9aa4af).into()
    }
    fn faint() -> Hsla {
        rgb(0x5f6873).into()
    }
    fn ok() -> Hsla {
        rgb(0x3fb950).into()
    }
    fn warn() -> Hsla {
        rgb(0xd29922).into()
    }
    fn bad() -> Hsla {
        rgb(0xf85149).into()
    }
    fn info() -> Hsla {
        rgb(0x58a6ff).into()
    }
    fn receive() -> Hsla {
        rgb(0x58a6ff).into()
    }
    fn send() -> Hsla {
        rgb(0xbc8cff).into()
    }
}

/// Scale function: design pixels to window pixels.
type Scale<'a> = &'a dyn Fn(f32) -> Pixels;

/// Per-window settings of the status view.
pub struct StatusViewSettings {
    /// Display description, such as `Display 1 of 2 · primary · 1920×1080`.
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
    pointer_moved_at: Option<Instant>,
    _subscriptions: Vec<Subscription>,
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
        let subscriptions = vec![
            cx.observe(&board, |_, _, cx| cx.notify()),
            cx.observe_window_activation(window, |_, _, cx| cx.notify()),
        ];
        Self {
            board,
            controller,
            settings,
            focus_handle,
            pointer_moved_at: None,
            _subscriptions: subscriptions,
        }
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

/// What a feed panel shows, with the feed's freshness applied.
struct FeedView<'a> {
    feed: &'a StatusFeed,
    freshness: FeedFreshness,
}

impl FeedView<'_> {
    /// `true` when the panel must show values in gray.
    fn dim(&self) -> bool {
        matches!(
            self.freshness,
            FeedFreshness::Stale | FeedFreshness::Offline
        )
    }

    /// The reading to show. Offline feeds show none.
    fn reading(&self) -> Option<&FeedData> {
        match self.freshness {
            FeedFreshness::Offline => None,
            _ => self.feed.latest_reading(),
        }
    }
}

impl Render for StatusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui = (f32::from(window.viewport_size().width) / DESIGN_WIDTH).clamp(0.6, 2.0);
        let scale = move |value: f32| px(value * ui);
        let s: Scale = &scale;
        let board = self.board.read(cx);
        let now = OffsetDateTime::now_utc();
        let offset = self.settings.local_offset;
        let feeds: Vec<FeedView> = board
            .status_feeds()
            .iter()
            .map(|feed| FeedView {
                feed,
                freshness: board.feed_freshness(feed),
            })
            .collect();

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
        for view in &feeds {
            if let Some(alert) = feed_alert(s, view) {
                alerts.push(alert.into_any_element());
            }
        }

        let clock = div()
            .flex()
            .flex_col()
            .font_family(STATUS_CLOCK_FONT_FAMILY)
            .child(
                div()
                    .text_size(s(128.))
                    .line_height(relative(1.))
                    .font_weight(FontWeight::LIGHT)
                    .child(format_status_clock(now, offset)),
            )
            .child(
                div()
                    .pl(s(6.))
                    .text_size(s(28.))
                    .child(format_status_date(now, offset)),
            );

        let is_fullscreen = window.is_fullscreen();
        let controls_visible = self
            .pointer_moved_at
            .is_some_and(|at| at.elapsed() < CONTROLS_VISIBLE_DURATION);
        let controls = div()
            .flex()
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

        let top = div()
            .flex_1()
            .min_h_0()
            .flex()
            .justify_between()
            .items_start()
            .gap(s(32.))
            .child(clock)
            .child(
                div()
                    .w(relative(0.5))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(s(8.))
                    .when(controls_visible, |column| column.child(controls))
                    .children(alerts),
            );

        let is_host = |view: &&FeedView| {
            matches!(
                view.feed.spec.collector,
                StatusCollector::Glances(_) | StatusCollector::Local(_)
            )
        };
        // Up to two hosts get one column each. More hosts share one column.
        let host_count = feeds.iter().filter(is_host).count();
        let host_rows = if host_count <= 2 { 1 } else { host_count };
        let panel_area = f32::from(window.viewport_size().height) * PANEL_AREA_HEIGHT / ui;
        let host_panel_height =
            (panel_area - PANEL_GAP * (host_rows as f32 - 1.0)) / host_rows.max(1) as f32;

        let mut host_panels = Vec::new();
        let mut service_panels = Vec::new();
        let mut side_panels = Vec::new();
        for view in &feeds {
            match &view.feed.spec.collector {
                StatusCollector::Glances(_) | StatusCollector::Local(_) => {
                    host_panels.push(host_panel(s, view, host_panel_height).into_any_element())
                }
                StatusCollector::Kuma(_) => {
                    service_panels.push(services_panel(s, view, now).into_any_element())
                }
                StatusCollector::Scrutiny(_) => {
                    side_panels.push(disks_panel(s, view).into_any_element())
                }
                StatusCollector::Tailscale(_) => {
                    side_panels.push(tailnet_panel(s, view, now).into_any_element())
                }
            }
        }
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
        let host_columns = if host_rows == 1 {
            let grow = if host_count == 1 { 1.3 } else { 1. };
            host_panels
                .into_iter()
                .filter_map(|panel| column(vec![panel], grow))
                .collect()
        } else {
            column(host_panels, 1.3).into_iter().collect::<Vec<_>>()
        };
        let panels = div()
            .h(relative(PANEL_AREA_HEIGHT))
            .flex()
            .gap(s(PANEL_GAP))
            .children(host_columns)
            .children(column(service_panels, 1.))
            .children(column(side_panels, 1.));

        div()
            .id("status-root")
            .key_context(STATUS_VIEW_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::exit_fullscreen))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::next_display))
            .on_mouse_move(cx.listener(|this, _, _, cx| {
                this.pointer_moved_at = Some(Instant::now());
                cx.notify();
            }))
            .relative()
            .size_full()
            .bg(StatusTheme::background())
            .font_family(STATUS_PANEL_FONT_FAMILY)
            .text_color(StatusTheme::text())
            .children(self.settings.wallpaper.clone().map(|path| {
                img(path)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
            }))
            .child(div().absolute().inset_0().bg(StatusTheme::scrim()))
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap(s(24.))
                    .p(s(32.))
                    .child(top)
                    .child(panels),
            )
    }
}

/// Alert for a feed that is stale or offline. Live feeds give none.
fn feed_alert(s: Scale, view: &FeedView) -> Option<Div> {
    let feed = view.feed;
    let name = format!("{} ({})", feed.spec.title, feed.spec.source);
    match view.freshness {
        FeedFreshness::Stale => Some(alert_row(
            s,
            StatusTheme::warn(),
            format!("{name} is not responding."),
            format!(
                "Data is {} old. {}",
                feed.last_success_age()
                    .map(format_status_age)
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
                .truncate()
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

/// Panel frame: title, one header fact or the freshness state, and body.
fn feed_panel(s: Scale, view: &FeedView, header_fact: Option<String>, body: AnyElement) -> Div {
    let (state_text, state_color) = match view.freshness {
        FeedFreshness::Waiting => (Some("connecting".to_string()), StatusTheme::info()),
        FeedFreshness::Live => (header_fact, StatusTheme::muted()),
        FeedFreshness::Stale => (
            view.feed
                .last_success_age()
                .map(|age| format!("stale {}", format_status_age(age))),
            StatusTheme::warn(),
        ),
        FeedFreshness::Offline => (Some("offline".to_string()), StatusTheme::bad()),
    };
    let border = match view.freshness {
        FeedFreshness::Stale => StatusTheme::warn().opacity(0.7),
        FeedFreshness::Offline => StatusTheme::bad().opacity(0.7),
        _ => StatusTheme::panel_border(),
    };
    div()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .gap(s(12.))
        .p(s(16.))
        .rounded(s(8.))
        .bg(StatusTheme::panel())
        .border_1()
        .border_color(border)
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap(s(12.))
                .child(
                    div()
                        .min_w_0()
                        .text_size(s(17.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(view.feed.spec.title.clone()),
                )
                .children(state_text.map(|text| {
                    div()
                        .flex_none()
                        .text_size(s(14.))
                        .text_color(state_color)
                        .child(text)
                })),
        )
        .child(body)
}

/// Body for a feed without data: waiting, or offline with the error.
fn empty_body(s: Scale, view: &FeedView) -> AnyElement {
    let text = match view.freshness {
        FeedFreshness::Offline => "No data",
        _ => "Waiting for data",
    };
    div()
        .flex_1()
        .flex()
        .flex_col()
        .justify_center()
        .gap(s(4.))
        .child(
            div()
                .text_size(s(16.))
                .text_color(StatusTheme::muted())
                .child(text),
        )
        .children(view.feed.last_poll_error().map(|error| {
            div()
                .text_size(s(13.))
                .text_color(StatusTheme::faint())
                .child(error.to_string())
        }))
        .into_any_element()
}

fn threshold_color(fraction: f32) -> Hsla {
    if fraction >= 0.9 {
        StatusTheme::bad()
    } else if fraction >= 0.75 {
        StatusTheme::warn()
    } else {
        StatusTheme::ok()
    }
}

fn value_color(dim: bool) -> Hsla {
    if dim {
        StatusTheme::muted()
    } else {
        StatusTheme::text()
    }
}

fn meter(s: Scale, fraction: Option<f32>, height: f32, dim: bool) -> Div {
    let fraction = fraction.map(|f| f.clamp(0.0, 1.0));
    let color = match (fraction, dim) {
        (_, true) | (None, _) => StatusTheme::faint(),
        (Some(fraction), false) => threshold_color(fraction),
    };
    div()
        .flex_1()
        .h(s(height))
        .rounded(s(height / 2.))
        .bg(StatusTheme::track())
        .child(
            div()
                .h_full()
                .rounded(s(height / 2.))
                .w(relative(fraction.unwrap_or(0.0)))
                .bg(color),
        )
}

fn metric_row(s: Scale, label: &str, fraction: Option<f32>, value: String, dim: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(12.))
        .text_size(s(14.))
        .child(
            div()
                .w(s(48.))
                .flex_none()
                .text_color(StatusTheme::muted())
                .child(label.to_string()),
        )
        .child(meter(s, fraction, 6., dim))
        .child(
            div()
                .w(s(152.))
                .flex_none()
                .flex()
                .justify_end()
                .text_color(value_color(dim))
                .child(value),
        )
}

/// CPU history graph: one bar for each sample, newest on the right.
fn cpu_graph(s: Scale, samples: &VecDeque<f32>, dim: bool) -> Div {
    let padding = HOST_HISTORY_SAMPLES.saturating_sub(samples.len());
    let bars = (0..padding).map(|_| None).chain(samples.iter().map(Some));
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_end()
        .gap(s(2.))
        .border_b_1()
        .border_color(StatusTheme::track())
        .children(bars.map(|sample| {
            let bar = div().flex_1().h_full().flex().items_end();
            match sample {
                Some(percent) => {
                    let fraction = (percent / 100.0).clamp(0.0, 1.0);
                    let color = if dim {
                        StatusTheme::faint()
                    } else {
                        threshold_color(fraction)
                    };
                    bar.child(
                        div()
                            .w_full()
                            .min_h(px(1.))
                            .h(relative(fraction))
                            .bg(color.opacity(0.85)),
                    )
                }
                None => bar,
            }
        }))
}

/// Lowest top of the network graph scale, so idle noise stays flat.
const NETWORK_GRAPH_FLOOR: f64 = 64.0 * 1024.0;

/// Network history graph: receive bars rise from the middle line and send
/// bars fall from it. The scale is the peak rate of the shown samples.
fn network_graph(s: Scale, history: &HostHistory, peak: f64, dim: bool) -> Div {
    let half = |samples: &VecDeque<f64>, color: Hsla, rising: bool| {
        let padding = HOST_HISTORY_SAMPLES.saturating_sub(samples.len());
        let bars = (0..padding).map(|_| None).chain(samples.iter().map(Some));
        let color = if dim { StatusTheme::faint() } else { color };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(s(2.))
            .map(|row| {
                if rising {
                    row.items_end()
                } else {
                    row.items_start()
                }
            })
            .children(bars.map(|rate| {
                let slot = div().flex_1().h_full().flex().map(|slot| {
                    if rising {
                        slot.items_end()
                    } else {
                        slot.items_start()
                    }
                });
                match rate {
                    Some(rate) => slot.child(
                        div()
                            .w_full()
                            .min_h(px(1.))
                            .h(relative((rate / peak).clamp(0.0, 1.0) as f32))
                            .bg(color.opacity(0.85)),
                    ),
                    None => slot,
                }
            }))
    };
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(half(
            &history.network_receive_rate,
            StatusTheme::receive(),
            true,
        ))
        .child(div().h(px(1.)).bg(StatusTheme::track()))
        .child(half(&history.network_send_rate, StatusTheme::send(), false))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .text_size(s(12.))
                .text_color(StatusTheme::faint())
                .child(format_status_rate(peak)),
        )
}

/// Per-core usage in a grid of small meters.
fn core_grid_columns(cores: &[f32]) -> usize {
    if cores.len() > 8 { 4 } else { 2 }
}

fn core_grid(s: Scale, cores: &[f32], dim: bool) -> Div {
    div()
        .grid()
        .grid_cols(core_grid_columns(cores) as u16)
        .gap_x(s(24.))
        .gap_y(s(6.))
        .text_size(s(13.))
        .children(cores.iter().enumerate().map(|(index, percent)| {
            div()
                .flex()
                .items_center()
                .gap(s(8.))
                .child(
                    div()
                        .w(s(24.))
                        .flex_none()
                        .text_color(StatusTheme::faint())
                        .child(index.to_string()),
                )
                .child(meter(s, Some(percent / 100.0), 4., dim))
                .child(
                    div()
                        .w(s(40.))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .text_color(StatusTheme::muted())
                        .child(format!("{percent:.0}%")),
                )
        }))
}

fn host_panel(s: Scale, view: &FeedView, panel_height: f32) -> Div {
    let Some(FeedData::Host(host)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view));
    };
    feed_panel(
        s,
        view,
        host.uptime_seconds
            .map(|up| format!("up {}", format_status_uptime(up))),
        host_body(s, host, view.feed.host_history(), panel_height, view.dim()).into_any_element(),
    )
}

fn host_body(
    s: Scale,
    host: &HostMetrics,
    history: &HostHistory,
    panel_height: f32,
    dim: bool,
) -> Div {
    let core_rows = host
        .core_percents
        .len()
        .div_ceil(core_grid_columns(&host.core_percents));
    let show_cores = !host.core_percents.is_empty()
        && panel_height >= HOST_PANEL_BASE_HEIGHT + CORE_GRID_ROW_HEIGHT * core_rows as f32;
    let ratio = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f32 / total as f32),
        _ => None,
    };
    let usage = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) => format_status_usage(used, total),
        _ => "—".into(),
    };
    let temperature_color = match host.temperature_celsius {
        _ if dim => StatusTheme::muted(),
        Some(t) if t >= 85.0 => StatusTheme::bad(),
        Some(t) if t >= 75.0 => StatusTheme::warn(),
        _ => StatusTheme::text(),
    };
    let peak = history
        .network_receive_rate
        .iter()
        .chain(&history.network_send_rate)
        .copied()
        .fold(NETWORK_GRAPH_FLOOR, f64::max);
    let rate_text = |rate: Option<f64>| rate.map_or("—".into(), format_status_rate);
    let network_header = div()
        .flex()
        .items_baseline()
        .gap(s(16.))
        .text_size(s(15.))
        .child(div().text_color(StatusTheme::muted()).child("Net"))
        .child(
            div()
                .text_color(if dim {
                    StatusTheme::muted()
                } else {
                    StatusTheme::receive()
                })
                .child(format!("↓ {}", rate_text(host.network_receive_rate))),
        )
        .child(
            div()
                .text_color(if dim {
                    StatusTheme::muted()
                } else {
                    StatusTheme::send()
                })
                .child(format!("↑ {}", rate_text(host.network_send_rate))),
        );
    let cpu_header = div()
        .flex()
        .items_baseline()
        .gap(s(16.))
        .text_size(s(15.))
        .child(div().text_color(StatusTheme::muted()).child("CPU"))
        .child(
            div()
                .text_size(s(28.))
                .text_color(value_color(dim))
                .child(host.cpu_percent.map_or("—".into(), |p| format!("{p:.0}%"))),
        )
        .child(div().flex_1())
        .children(host.temperature_celsius.map(|t| {
            div()
                .text_color(temperature_color)
                .child(format!("{t:.0}°C"))
        }))
        .children(host.load_1m.map(|load| {
            div()
                .text_color(StatusTheme::muted())
                .child(format!("load {load:.2}"))
        }));
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap(s(12.))
        .child(cpu_header)
        .child(cpu_graph(s, &history.cpu_percent, dim))
        .when(show_cores, |body| {
            body.child(core_grid(s, &host.core_percents, dim))
        })
        .child(network_header)
        .child(network_graph(s, history, peak, dim))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(s(8.))
                .child(metric_row(
                    s,
                    "Mem",
                    ratio(host.memory_used_bytes, host.memory_total_bytes),
                    usage(host.memory_used_bytes, host.memory_total_bytes),
                    dim,
                ))
                .child(metric_row(
                    s,
                    "Disk",
                    ratio(host.disk_used_bytes, host.disk_total_bytes),
                    usage(host.disk_used_bytes, host.disk_total_bytes),
                    dim,
                )),
        )
}

fn status_dot(s: Scale, color: Hsla) -> Div {
    div().flex_none().size(s(8.)).rounded(s(4.)).bg(color)
}

/// List row: state dot, name, then right-aligned columns of fixed width.
fn list_row(s: Scale, color: Hsla, name: String, columns: Vec<(String, f32, Hsla)>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(12.))
        .text_size(s(14.))
        .child(status_dot(s, color))
        .child(div().flex_1().min_w_0().truncate().child(name))
        .children(columns.into_iter().map(|(text, width, color)| {
            div()
                .flex_none()
                .w(s(width))
                .flex()
                .justify_end()
                .text_color(color)
                .child(text)
        }))
}

fn list_body(s: Scale, rows: impl IntoIterator<Item = Div>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(s(8.))
        .children(rows)
        .into_any_element()
}

fn age_since(now: OffsetDateTime, at: Option<OffsetDateTime>) -> String {
    at.map(|at| {
        let age = Duration::try_from(now - at).unwrap_or_default();
        format!("{} ago", format_status_age(age))
    })
    .unwrap_or_else(|| "—".into())
}

fn services_panel(s: Scale, view: &FeedView, now: OffsetDateTime) -> Div {
    let Some(FeedData::Services(services)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view));
    };
    let mut services: Vec<&ServiceHealth> = services.iter().collect();
    let rank = |state: ServiceState| match state {
        ServiceState::Down => 0,
        ServiceState::Pending => 1,
        ServiceState::Unknown => 2,
        ServiceState::Maintenance => 3,
        ServiceState::Up => 4,
    };
    services.sort_by(|a, b| rank(a.state).cmp(&rank(b.state)).then(a.name.cmp(&b.name)));
    let up = services
        .iter()
        .filter(|s| s.state == ServiceState::Up)
        .count();
    let total = services.len();
    let dim = view.dim();
    let rows = services.into_iter().map(|service| {
        let color = match (dim, service.state) {
            (true, _) => StatusTheme::faint(),
            (_, ServiceState::Up) => StatusTheme::ok(),
            (_, ServiceState::Down) => StatusTheme::bad(),
            (_, ServiceState::Pending) => StatusTheme::warn(),
            (_, ServiceState::Maintenance) => StatusTheme::info(),
            (_, ServiceState::Unknown) => StatusTheme::faint(),
        };
        let uptime = service.uptime_24h.map_or("—".into(), |ratio| {
            if ratio >= 0.9995 {
                "100%".into()
            } else {
                format!("{:.1}%", ratio * 100.0)
            }
        });
        let state = match service.state {
            ServiceState::Up => None,
            ServiceState::Down => Some("down"),
            ServiceState::Pending => Some("pending"),
            ServiceState::Maintenance => Some("maintenance"),
            ServiceState::Unknown => Some("no data"),
        };
        let mut columns = Vec::new();
        if let Some(state) = state {
            columns.push((
                format!("{state} {}", age_since(now, service.last_heartbeat)),
                176.,
                color,
            ));
        }
        columns.push((uptime, 64., StatusTheme::muted()));
        list_row(s, color, service.name.clone(), columns)
    });
    feed_panel(
        s,
        view,
        Some(format!("{up}/{total} up")),
        list_body(s, rows),
    )
}

fn disks_panel(s: Scale, view: &FeedView) -> Div {
    let Some(FeedData::Disks(disks)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view));
    };
    let dim = view.dim();
    let passed = disks
        .iter()
        .filter(|d| d.state == DiskState::Passed)
        .count();
    let rows = disks.iter().map(|disk: &DiskHealth| {
        let (color, state) = match disk.state {
            DiskState::Passed => (StatusTheme::ok(), "passed"),
            DiskState::FailedSmart => (StatusTheme::bad(), "SMART failed"),
            DiskState::FailedScrutiny => (StatusTheme::warn(), "threshold"),
            DiskState::FailedBoth => (StatusTheme::bad(), "failed"),
        };
        let color = if dim { StatusTheme::faint() } else { color };
        let state_color = if dim || disk.state == DiskState::Passed {
            StatusTheme::muted()
        } else {
            color
        };
        list_row(
            s,
            color,
            disk.device.clone(),
            vec![
                (
                    disk.temperature_celsius
                        .map_or(String::new(), |t| format!("{t:.0}°C")),
                    48.,
                    StatusTheme::muted(),
                ),
                (
                    format_status_bytes(disk.capacity_bytes),
                    72.,
                    StatusTheme::muted(),
                ),
                (state.to_string(), 88., state_color),
            ],
        )
    });
    feed_panel(
        s,
        view,
        Some(format!("{passed}/{} passed", disks.len())),
        list_body(s, rows),
    )
}

fn tailnet_panel(s: Scale, view: &FeedView, now: OffsetDateTime) -> Div {
    let Some(FeedData::Tailnet(devices)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view));
    };
    let dim = view.dim();
    let online = devices.iter().filter(|d| d.online).count();
    let rows = devices.iter().map(|device: &TailnetDevice| {
        let color = match (dim, device.online) {
            (true, _) | (_, false) => StatusTheme::faint(),
            (_, true) => StatusTheme::ok(),
        };
        let connection = match &device.connection {
            TailnetConnection::ThisMachine => "this machine".to_string(),
            TailnetConnection::Direct => "direct".to_string(),
            TailnetConnection::Relay(relay) => format!("relay {relay}"),
            TailnetConnection::Idle => "idle".to_string(),
            TailnetConnection::Offline => match device.last_seen {
                Some(_) => age_since(now, device.last_seen),
                None => "offline".to_string(),
            },
        };
        list_row(
            s,
            color,
            device.name.clone(),
            vec![
                (device.os.clone(), 64., StatusTheme::muted()),
                (connection, 104., StatusTheme::muted()),
            ],
        )
        .when(!device.online, |row| row.text_color(StatusTheme::muted()))
    });
    feed_panel(
        s,
        view,
        Some(format!("{online}/{} online", devices.len())),
        list_body(s, rows),
    )
}
