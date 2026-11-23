//! Status view: the full-screen GPUI layout for the status display.

use std::time::Duration;

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, FocusHandle, FontWeight, Hsla, IntoElement,
    Pixels, Render, Stateful, Subscription, WeakEntity, Window, actions, div, prelude::*, px,
    relative, rgb,
};
use time::{OffsetDateTime, UtcOffset};

use crate::{
    status_board::{StatusBoard, StatusFeed},
    status_collector::StatusCollector,
    status_fonts::STATUS_FONT_FAMILY,
    status_format::{
        format_status_age, format_status_bytes, format_status_clock, format_status_date,
        format_status_uptime, format_status_usage,
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

/// Keyboard shortcut summary for the footer.
#[cfg(target_os = "macos")]
const STATUS_KEY_HINTS: &str =
    "F11 fullscreen · Esc windowed · D next display · R refresh · Cmd+Q quit";
#[cfg(not(target_os = "macos"))]
const STATUS_KEY_HINTS: &str =
    "F11 fullscreen · Esc windowed · D next display · R refresh · Ctrl+Q quit";

/// Design reference width. The layout scales from this width.
const DESIGN_WIDTH: f32 = 1600.0;

struct StatusTheme;

impl StatusTheme {
    fn background() -> Hsla {
        rgb(0x0b0f14).into()
    }
    fn card() -> Hsla {
        rgb(0x141a22).into()
    }
    fn card_border() -> Hsla {
        rgb(0x222b36).into()
    }
    fn track() -> Hsla {
        rgb(0x26303c).into()
    }
    fn text() -> Hsla {
        rgb(0xe6edf3).into()
    }
    fn muted() -> Hsla {
        rgb(0x8b97a5).into()
    }
    fn faint() -> Hsla {
        rgb(0x56616e).into()
    }
    fn ok() -> Hsla {
        rgb(0x3fb950).into()
    }
    fn warn() -> Hsla {
        rgb(0xe3b341).into()
    }
    fn bad() -> Hsla {
        rgb(0xf85149).into()
    }
    fn info() -> Hsla {
        rgb(0x58a6ff).into()
    }
}

/// Scale function: design pixels to window pixels.
type Scale<'a> = &'a dyn Fn(f32) -> Pixels;

/// Status view: header, alerts, host cards, services, disks, and tailnet.
pub struct StatusView {
    board: Entity<StatusBoard>,
    controller: WeakEntity<StatusDisplayApp>,
    display_label: String,
    local_offset: UtcOffset,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl StatusView {
    /// Creates the status view for one window.
    pub fn new_status_view(
        board: Entity<StatusBoard>,
        controller: WeakEntity<StatusDisplayApp>,
        display_label: String,
        local_offset: UtcOffset,
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
            display_label,
            local_offset,
            focus_handle,
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
        let has_focus = window.is_window_active();
        let is_fullscreen = window.is_fullscreen();
        let feeds: Vec<FeedView> = board
            .status_feeds()
            .iter()
            .map(|feed| FeedView {
                feed,
                freshness: board.feed_freshness(feed),
            })
            .collect();

        let mut alerts: Vec<AnyElement> = Vec::new();
        if has_focus {
            alerts.push(
                alert_bar(
                    s,
                    StatusTheme::warn(),
                    "This window has keyboard focus.",
                    "Remote input from Moonlight now goes here, not to the game. Click the game or use Alt+Tab on the host.",
                )
                .into_any_element(),
            );
        }
        for view in &feeds {
            let feed = view.feed;
            let name = format!("{} ({})", feed.spec.title, feed.spec.source);
            let error = feed.last_poll_error().unwrap_or("");
            match view.freshness {
                FeedFreshness::Stale => alerts.push(
                    alert_bar(
                        s,
                        StatusTheme::warn(),
                        &format!("{name} is not responding."),
                        &format!(
                            "Showing data from {} ago. {} failed polls. {error}",
                            feed.last_success_age()
                                .map(format_status_age)
                                .unwrap_or_default(),
                            feed.consecutive_failures(),
                        ),
                    )
                    .into_any_element(),
                ),
                FeedFreshness::Offline => alerts.push(
                    alert_bar(
                        s,
                        StatusTheme::bad(),
                        &format!("{name} is offline."),
                        &format!(
                            "No data for {}. Retrying every {}. {error}",
                            feed.last_success_age()
                                .map(format_status_age)
                                .unwrap_or_else(|| "this session".into()),
                            format_status_age(feed.policy.poll_interval),
                        ),
                    )
                    .into_any_element(),
                ),
                FeedFreshness::Waiting | FeedFreshness::Live => {}
            }
        }

        let live = feeds
            .iter()
            .filter(|v| v.freshness == FeedFreshness::Live)
            .count();
        let worst = feeds
            .iter()
            .map(|v| v.freshness)
            .max_by_key(|f| match f {
                FeedFreshness::Live => 0,
                FeedFreshness::Waiting => 1,
                FeedFreshness::Stale => 2,
                FeedFreshness::Offline => 3,
            })
            .unwrap_or(FeedFreshness::Waiting);
        let (pill_color, pill_text) = match worst {
            FeedFreshness::Live => (StatusTheme::ok(), "ALL LIVE".to_string()),
            FeedFreshness::Waiting => (StatusTheme::info(), "CONNECTING".to_string()),
            FeedFreshness::Stale => (StatusTheme::warn(), "STALE DATA".to_string()),
            FeedFreshness::Offline => (StatusTheme::bad(), "SOURCE OFFLINE".to_string()),
        };
        let summary = overall_summary(&feeds);

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(s(24.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(s(4.))
                    .child(
                        div()
                            .text_size(s(30.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Server status"),
                    )
                    .child(
                        div()
                            .text_size(s(17.))
                            .text_color(StatusTheme::muted())
                            .child(summary),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(s(6.))
                    .child(pill(s, pill_color, pill_text, 18.))
                    .child(
                        div()
                            .text_size(s(15.))
                            .text_color(StatusTheme::muted())
                            .child(format!("{live} of {} sources live", feeds.len())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(s(2.))
                    .child(
                        div()
                            .text_size(s(40.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format_status_clock(now, self.local_offset)),
                    )
                    .child(
                        div()
                            .text_size(s(16.))
                            .text_color(StatusTheme::muted())
                            .child(format_status_date(now, self.local_offset)),
                    ),
            );

        let controls = div()
            .flex()
            .gap(s(8.))
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

        let mut host_panels = Vec::new();
        let mut service_panels = Vec::new();
        let mut lower_panels = Vec::new();
        for view in &feeds {
            match &view.feed.spec.collector {
                StatusCollector::Glances(_) | StatusCollector::Local(_) => {
                    host_panels.push(host_panel(s, view).into_any_element())
                }
                StatusCollector::Kuma(_) => {
                    service_panels.push(services_panel(s, view, now).into_any_element())
                }
                StatusCollector::Scrutiny(_) => {
                    lower_panels.push(disks_panel(s, view).into_any_element())
                }
                StatusCollector::Tailscale(_) => {
                    lower_panels.push(tailnet_panel(s, view, now).into_any_element())
                }
            }
        }

        let left = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(s(16.))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(s(16.))
                    .children(host_panels),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(s(16.))
                    .children(lower_panels),
            );
        let main = div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(s(16.))
            .child(left)
            .when(!service_panels.is_empty(), |main| {
                main.child(
                    div()
                        .w(relative(0.3))
                        .flex()
                        .flex_col()
                        .children(service_panels),
                )
            });

        let footer = div()
            .flex()
            .justify_between()
            .gap(s(16.))
            .text_size(s(14.))
            .text_color(StatusTheme::faint())
            .child(div().child(self.display_label.clone()))
            .child(div().child(STATUS_KEY_HINTS));

        div()
            .id("status-root")
            .key_context(STATUS_VIEW_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::exit_fullscreen))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::next_display))
            .size_full()
            .flex()
            .flex_col()
            .gap(s(16.))
            .p(s(28.))
            .bg(StatusTheme::background())
            .font_family(STATUS_FONT_FAMILY)
            .text_color(StatusTheme::text())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(s(6.))
                    .child(div().flex().justify_end().child(controls))
                    .child(header),
            )
            .children(alerts)
            .child(main)
            .child(footer)
    }
}

fn overall_summary(feeds: &[FeedView]) -> String {
    let mut parts = Vec::new();
    let hosts = feeds
        .iter()
        .filter(|v| {
            matches!(
                v.feed.spec.collector,
                StatusCollector::Glances(_) | StatusCollector::Local(_)
            )
        })
        .count();
    if hosts > 0 {
        parts.push(format!("{hosts} hosts"));
    }
    for view in feeds {
        match view.feed.latest_reading() {
            Some(FeedData::Services(services)) => {
                let up = services
                    .iter()
                    .filter(|s| s.state == ServiceState::Up)
                    .count();
                parts.push(format!("{up}/{} services up", services.len()));
            }
            Some(FeedData::Disks(disks)) => {
                let passed = disks
                    .iter()
                    .filter(|d| d.state == DiskState::Passed)
                    .count();
                parts.push(format!("{passed}/{} disks passed", disks.len()));
            }
            Some(FeedData::Tailnet(devices)) => {
                let online = devices.iter().filter(|d| d.online).count();
                parts.push(format!("{online}/{} tailnet devices online", devices.len()));
            }
            _ => {}
        }
    }
    parts.join(" · ")
}

fn pill(s: Scale, color: Hsla, text: String, size: f32) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(size * 0.5))
        .px(s(size * 0.8))
        .py(s(size * 0.3))
        .rounded(s(999.))
        .bg(color.opacity(0.14))
        .border_1()
        .border_color(color.opacity(0.5))
        .child(div().size(s(size * 0.6)).rounded(s(size)).bg(color))
        .child(
            div()
                .text_size(s(size))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(text),
        )
}

fn alert_bar(s: Scale, color: Hsla, title: &str, detail: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(14.))
        .px(s(18.))
        .py(s(10.))
        .rounded(s(10.))
        .bg(color.opacity(0.12))
        .border_1()
        .border_color(color.opacity(0.6))
        .child(div().size(s(12.)).rounded(s(6.)).bg(color))
        .child(
            div()
                .text_size(s(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(title.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(s(16.))
                .truncate()
                .child(detail.to_string()),
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
        .border_1()
        .border_color(StatusTheme::card_border())
        .text_size(s(13.))
        .text_color(StatusTheme::muted())
        .cursor_pointer()
        .hover(|style| {
            style
                .bg(StatusTheme::card())
                .text_color(StatusTheme::text())
        })
        .on_click(on_click)
        .child(label)
}

/// Panel frame: title, source, freshness badge, and body.
fn feed_panel(s: Scale, view: &FeedView, subtitle: Option<String>, body: AnyElement) -> Div {
    let feed = view.feed;
    let (badge_color, badge_text) = match view.freshness {
        FeedFreshness::Waiting => (StatusTheme::info(), "CONNECTING".to_string()),
        FeedFreshness::Live => (StatusTheme::ok(), "LIVE".to_string()),
        FeedFreshness::Stale => (
            StatusTheme::warn(),
            format!(
                "STALE · {}",
                feed.last_success_age()
                    .map(format_status_age)
                    .unwrap_or_default()
            ),
        ),
        FeedFreshness::Offline => (StatusTheme::bad(), "OFFLINE".to_string()),
    };
    let border = match view.freshness {
        FeedFreshness::Stale => StatusTheme::warn().opacity(0.6),
        FeedFreshness::Offline => StatusTheme::bad().opacity(0.7),
        _ => StatusTheme::card_border(),
    };
    div()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .gap(s(12.))
        .p(s(18.))
        .rounded(s(14.))
        .bg(StatusTheme::card())
        .border_1()
        .border_color(border)
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(s(12.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .gap(s(2.))
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(s(10.))
                                .min_w_0()
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_size(s(24.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .truncate()
                                        .child(feed.spec.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(s(14.))
                                        .flex_none()
                                        .text_color(StatusTheme::faint())
                                        .child(feed.spec.source),
                                ),
                        )
                        .children(subtitle.map(|subtitle| {
                            div()
                                .text_size(s(14.))
                                .text_color(StatusTheme::muted())
                                .truncate()
                                .child(subtitle)
                        })),
                )
                .child(pill(s, badge_color, badge_text, 13.).flex_none()),
        )
        .child(body)
}

/// Body for a feed without data: waiting, or offline with the error.
fn empty_body(s: Scale, view: &FeedView) -> AnyElement {
    let (text, color) = match view.freshness {
        FeedFreshness::Offline => ("No current data", StatusTheme::bad()),
        _ => ("Waiting for the first reading…", StatusTheme::muted()),
    };
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(s(6.))
        .child(div().text_size(s(20.)).text_color(color).child(text))
        .children(view.feed.last_poll_error().map(|error| {
            div()
                .text_size(s(14.))
                .text_color(StatusTheme::muted())
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

fn metric_bar(s: Scale, label: String, fraction: Option<f32>, value: String, dim: bool) -> Div {
    let fraction = fraction.map(|f| f.clamp(0.0, 1.0));
    let color = match (fraction, dim) {
        (_, true) | (None, _) => StatusTheme::faint(),
        (Some(fraction), false) => threshold_color(fraction),
    };
    div()
        .flex()
        .items_center()
        .gap(s(12.))
        .child(
            div()
                .w(s(84.))
                .text_size(s(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(StatusTheme::muted())
                .truncate()
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .h(s(10.))
                .rounded(s(5.))
                .bg(StatusTheme::track())
                .child(
                    div()
                        .h_full()
                        .rounded(s(5.))
                        .w(relative(fraction.unwrap_or(0.0)))
                        .bg(color),
                ),
        )
        .child(
            div()
                .w(s(150.))
                .flex()
                .justify_end()
                .text_size(s(17.))
                .text_color(if dim {
                    StatusTheme::muted()
                } else {
                    StatusTheme::text()
                })
                .child(value),
        )
}

fn stat(s: Scale, label: &str, value: String, color: Hsla) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(s(8.))
        .child(
            div()
                .text_size(s(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(StatusTheme::muted())
                .child(label.to_string()),
        )
        .child(div().text_size(s(17.)).text_color(color).child(value))
}

fn host_panel(s: Scale, view: &FeedView) -> Div {
    let Some(FeedData::Host(host)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view));
    };
    let title_detail = if view.feed.spec.title == host.name {
        host.detail.clone()
    } else {
        format!("{} · {}", host.name, host.detail)
    };
    feed_panel(
        s,
        view,
        Some(title_detail),
        host_body(s, host, view.dim()).into_any_element(),
    )
}

fn host_body(s: Scale, host: &HostMetrics, dim: bool) -> Div {
    let ratio = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f32 / total as f32),
        _ => None,
    };
    let usage = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) => format_status_usage(used, total),
        _ => "—".into(),
    };
    let value_color = if dim {
        StatusTheme::muted()
    } else {
        StatusTheme::text()
    };
    let temperature_color = match host.temperature_celsius {
        _ if dim => StatusTheme::muted(),
        Some(t) if t >= 85.0 => StatusTheme::bad(),
        Some(t) if t >= 75.0 => StatusTheme::warn(),
        _ => StatusTheme::text(),
    };
    let cpu_label = match host.cpu_cores {
        Some(cores) => format!("CPU ×{cores}"),
        None => "CPU".into(),
    };
    div()
        .flex()
        .flex_col()
        .gap(s(12.))
        .child(metric_bar(
            s,
            cpu_label,
            host.cpu_percent.map(|p| p / 100.0),
            host.cpu_percent.map_or("—".into(), |p| format!("{p:.0} %")),
            dim,
        ))
        .child(metric_bar(
            s,
            "MEM".into(),
            ratio(host.memory_used_bytes, host.memory_total_bytes),
            usage(host.memory_used_bytes, host.memory_total_bytes),
            dim,
        ))
        .child(metric_bar(
            s,
            "DISK".into(),
            ratio(host.disk_used_bytes, host.disk_total_bytes),
            usage(host.disk_used_bytes, host.disk_total_bytes),
            dim,
        ))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(s(22.))
                .gap_y(s(6.))
                .child(stat(
                    s,
                    "TEMP",
                    host.temperature_celsius
                        .map_or("—".into(), |t| format!("{t:.0} °C")),
                    temperature_color,
                ))
                .child(stat(
                    s,
                    "LOAD",
                    host.load_1m.map_or("—".into(), |l| format!("{l:.2}")),
                    value_color,
                ))
                .child(stat(
                    s,
                    "UP",
                    host.uptime_seconds.map_or("—".into(), format_status_uptime),
                    value_color,
                )),
        )
}

fn status_dot(s: Scale, color: Hsla) -> Div {
    div().flex_none().size(s(10.)).rounded(s(5.)).bg(color)
}

fn list_row(s: Scale, color: Hsla, name: String, middle: String, right: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(10.))
        .py(s(1.))
        .child(status_dot(s, color))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(s(16.))
                .truncate()
                .child(name),
        )
        .child(
            div()
                .flex_none()
                .text_size(s(15.))
                .text_color(StatusTheme::muted())
                .child(middle),
        )
        .child(
            div()
                .flex_none()
                .w(s(130.))
                .flex()
                .justify_end()
                .text_size(s(15.))
                .text_color(StatusTheme::muted())
                .child(right),
        )
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
        let name = match (&service.message, service.state) {
            (Some(message), state) if state != ServiceState::Up => {
                format!("{} — {message}", service.name)
            }
            _ => service.name.clone(),
        };
        list_row(
            s,
            color,
            name,
            service
                .uptime_24h
                .map_or("—".into(), |ratio| format!("{:.1} %", ratio * 100.0)),
            age_since(now, service.last_heartbeat),
        )
    });
    feed_panel(
        s,
        view,
        Some(format!("{up} of {total} up · 24 h uptime · last check")),
        div()
            .flex()
            .flex_col()
            .gap(s(2.))
            .children(rows)
            .into_any_element(),
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
            DiskState::FailedScrutiny => (StatusTheme::warn(), "threshold failed"),
            DiskState::FailedBoth => (StatusTheme::bad(), "failed"),
        };
        let color = if dim { StatusTheme::faint() } else { color };
        let details = [
            Some(disk.model.clone()),
            Some(format_status_bytes(disk.capacity_bytes)),
            disk.temperature_celsius.map(|t| format!("{t:.0} °C")),
            disk.power_on_hours
                .map(|hours| format!("{:.1} y on", hours as f64 / 8_766.0)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        div()
            .flex()
            .items_center()
            .gap(s(10.))
            .py(s(3.))
            .child(status_dot(s, color))
            .child(
                div()
                    .flex_none()
                    .w(s(72.))
                    .text_size(s(17.))
                    .child(disk.device.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(s(15.))
                    .text_color(StatusTheme::muted())
                    .truncate()
                    .child(details),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(s(15.))
                    .text_color(if dim || disk.state == DiskState::Passed {
                        StatusTheme::muted()
                    } else {
                        color
                    })
                    .child(state),
            )
    });
    feed_panel(
        s,
        view,
        Some(format!("{passed} of {} passed SMART checks", disks.len())),
        div()
            .flex()
            .flex_col()
            .gap(s(2.))
            .children(rows)
            .into_any_element(),
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
            (true, _) => StatusTheme::faint(),
            (_, true) => StatusTheme::ok(),
            (_, false) => StatusTheme::faint(),
        };
        let connection = match &device.connection {
            TailnetConnection::ThisMachine => "this machine".to_string(),
            TailnetConnection::Direct => "direct".to_string(),
            TailnetConnection::Relay(relay) => format!("relay {relay}"),
            TailnetConnection::Idle => "idle".to_string(),
            TailnetConnection::Offline => match device.last_seen {
                Some(_) => format!("seen {}", age_since(now, device.last_seen)),
                None => "offline".to_string(),
            },
        };
        list_row(s, color, device.name.clone(), device.os.clone(), connection)
            .when(device.connection == TailnetConnection::Offline, |row| {
                row.text_color(StatusTheme::muted())
            })
    });
    feed_panel(
        s,
        view,
        Some(format!("{online} of {} devices online", devices.len())),
        div()
            .flex()
            .flex_col()
            .gap(s(2.))
            .children(rows)
            .into_any_element(),
    )
}
