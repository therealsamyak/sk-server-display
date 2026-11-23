//! Status view: the full-screen GPUI layout for the status display.

use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, FocusHandle, FontWeight, Hsla, IntoElement,
    Pixels, Render, Stateful, Subscription, WeakEntity, Window, actions, div, prelude::*, px,
    relative, rgb,
};
use time::{OffsetDateTime, UtcOffset};

use crate::{
    status_board::StatusBoard,
    status_fonts::STATUS_FONT_FAMILY,
    status_format::{
        format_status_age, format_status_clock, format_status_date, format_status_rate,
        format_status_uptime, format_status_usage,
    },
    status_freshness::{HostFreshness, SourceFreshness, classify_host_freshness},
    status_snapshot::{HostStatus, ServiceState, ServiceStatus, StatusSnapshot},
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
        /// Polls the status source now.
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

/// Status view: header, alerts, host cards, services, and footer.
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
            .update(cx, |board, cx| board.refresh_status_now(cx));
    }

    fn next_display(&mut self, _: &NextStatusDisplay, _: &mut Window, cx: &mut Context<Self>) {
        self.controller
            .update(cx, |controller, cx| controller.cycle_status_display(cx))
            .ok();
    }
}

impl Render for StatusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui = (f32::from(window.viewport_size().width) / DESIGN_WIDTH).clamp(0.6, 2.0);
        let s = |value: f32| px(value * ui);
        let board = self.board.read(cx);
        let now = OffsetDateTime::now_utc();
        let freshness = board.source_freshness();
        let policy = *board.freshness_policy();
        let snapshot = board.latest_snapshot().cloned();
        let has_focus = window.is_window_active();
        let is_fullscreen = window.is_fullscreen();

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
        match freshness {
            SourceFreshness::Stale => alerts.push(
                alert_bar(
                    s,
                    StatusTheme::warn(),
                    "Status source is not responding.",
                    &format!(
                        "Showing data from {} ago. {} failed polls. {}",
                        board
                            .last_success_age()
                            .map(format_status_age)
                            .unwrap_or_default(),
                        board.consecutive_failures(),
                        board.last_poll_error().unwrap_or(""),
                    ),
                )
                .into_any_element(),
            ),
            SourceFreshness::Offline => alerts.push(
                alert_bar(
                    s,
                    StatusTheme::bad(),
                    "Status source offline.",
                    &format!(
                        "No data for {}. Retrying every {}. {}",
                        board
                            .last_success_age()
                            .map(format_status_age)
                            .unwrap_or_else(|| "this session".into()),
                        format_status_age(policy.poll_interval),
                        board.last_poll_error().unwrap_or(""),
                    ),
                )
                .into_any_element(),
            ),
            SourceFreshness::Waiting | SourceFreshness::Live => {}
        }

        let (pill_color, pill_text) = match freshness {
            SourceFreshness::Waiting => (StatusTheme::info(), "CONNECTING".to_string()),
            SourceFreshness::Live => (StatusTheme::ok(), "LIVE".to_string()),
            SourceFreshness::Stale => (StatusTheme::warn(), "STALE".to_string()),
            SourceFreshness::Offline => (StatusTheme::bad(), "OFFLINE".to_string()),
        };
        let updated_text = match board.last_success_wall() {
            Some(at) => format!(
                "updated {} · {} ago · every {}",
                format_status_clock(at, self.local_offset),
                board
                    .last_success_age()
                    .map(format_status_age)
                    .unwrap_or_default(),
                format_status_age(policy.poll_interval)
            ),
            None => format!(
                "waiting for first poll · every {}",
                format_status_age(policy.poll_interval)
            ),
        };
        let summary = snapshot
            .as_ref()
            .map(|snapshot| match freshness {
                SourceFreshness::Stale | SourceFreshness::Offline => {
                    format!("Last known: {}", snapshot_summary(snapshot, &policy))
                }
                SourceFreshness::Waiting | SourceFreshness::Live => {
                    snapshot_summary(snapshot, &policy)
                }
            })
            .unwrap_or_else(|| "No data yet".into());

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
                            .text_color(StatusTheme::text())
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(s(10.))
                            .px(s(16.))
                            .py(s(6.))
                            .rounded(s(999.))
                            .bg(pill_color.opacity(0.14))
                            .border_1()
                            .border_color(pill_color.opacity(0.5))
                            .child(div().size(s(12.)).rounded(s(6.)).bg(pill_color))
                            .child(
                                div()
                                    .text_size(s(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(pill_color)
                                    .child(pill_text),
                            ),
                    )
                    .child(
                        div()
                            .text_size(s(15.))
                            .text_color(StatusTheme::muted())
                            .child(updated_text),
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
                            .text_color(StatusTheme::text())
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

        let source_offline = freshness == SourceFreshness::Offline;
        let source_stale = freshness == SourceFreshness::Stale;
        let host_cards: Vec<AnyElement> = match &snapshot {
            Some(snapshot) => snapshot
                .hosts
                .iter()
                .map(|host| {
                    let host_freshness =
                        classify_host_freshness(&policy, host, snapshot.generated_at);
                    host_card(
                        s,
                        host,
                        host_freshness,
                        snapshot.generated_at,
                        source_stale,
                        source_offline,
                    )
                    .into_any_element()
                })
                .collect(),
            None => vec![
                div()
                    .col_span_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(s(22.))
                    .text_color(StatusTheme::muted())
                    .child(if source_offline {
                        "No status data. The status source did not answer."
                    } else {
                        "Waiting for the first status snapshot…"
                    })
                    .into_any_element(),
            ],
        };
        let host_count = snapshot.as_ref().map_or(1, |s| s.hosts.len().max(1));
        let columns = if host_count <= 4 { 2 } else { 3 };

        let services = snapshot
            .as_ref()
            .map(|snapshot| snapshot.services.clone())
            .unwrap_or_default();
        let services_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(s(10.))
            .child(
                div()
                    .text_size(s(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(StatusTheme::muted())
                    .mr(s(6.))
                    .child("SERVICES"),
            )
            .children(
                services
                    .iter()
                    .map(|service| service_pill(s, service, source_stale || source_offline)),
            );

        let footer = div()
            .flex()
            .justify_between()
            .gap(s(16.))
            .text_size(s(14.))
            .text_color(StatusTheme::faint())
            .child(div().child(format!(
                "{} · source: {}",
                self.display_label,
                board.source_label()
            )))
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
            .gap(s(18.))
            .p(s(28.))
            .bg(StatusTheme::background())
            .font_family(STATUS_FONT_FAMILY)
            .text_color(StatusTheme::text())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(s(10.))
                    .child(div().flex().justify_end().child(controls))
                    .child(header),
            )
            .children(alerts)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .grid()
                    .grid_cols(columns)
                    .gap(s(16.))
                    .children(host_cards),
            )
            .child(services_row)
            .child(footer)
    }
}

fn snapshot_summary(
    snapshot: &StatusSnapshot,
    policy: &crate::status_freshness::FreshnessPolicy,
) -> String {
    let hosts = snapshot.hosts.len();
    let live = snapshot
        .hosts
        .iter()
        .filter(|host| {
            classify_host_freshness(policy, host, snapshot.generated_at) == HostFreshness::Live
        })
        .count();
    let down = snapshot.hosts.iter().filter(|host| !host.reachable).count();
    let services_ok = snapshot
        .services
        .iter()
        .filter(|service| service.state == ServiceState::Ok)
        .count();
    format!(
        "{hosts} hosts · {live} live · {} stale · {down} down · {services_ok}/{} services OK",
        hosts - live - down,
        snapshot.services.len()
    )
}

fn alert_bar(s: impl Fn(f32) -> Pixels, color: Hsla, title: &str, detail: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(14.))
        .px(s(18.))
        .py(s(12.))
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
                .text_size(s(16.))
                .text_color(StatusTheme::text())
                .truncate()
                .child(detail.to_string()),
        )
}

fn control_button(
    s: impl Fn(f32) -> Pixels,
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
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

fn threshold_color(fraction: f32) -> Hsla {
    if fraction >= 0.9 {
        StatusTheme::bad()
    } else if fraction >= 0.75 {
        StatusTheme::warn()
    } else {
        StatusTheme::ok()
    }
}

fn metric_bar(
    s: &impl Fn(f32) -> Pixels,
    label: &str,
    fraction: Option<f32>,
    value: String,
    dim: bool,
) -> Div {
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
                .w(s(52.))
                .text_size(s(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(StatusTheme::muted())
                .child(label.to_string()),
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

fn host_card(
    s: impl Fn(f32) -> Pixels,
    host: &HostStatus,
    freshness: HostFreshness,
    generated_at: OffsetDateTime,
    source_stale: bool,
    source_offline: bool,
) -> Div {
    let sample_age =
        std::time::Duration::try_from(generated_at - host.sampled_at).unwrap_or_default();
    let (badge_color, badge_text) = match (source_offline, freshness) {
        (true, _) => (StatusTheme::faint(), "NO DATA".to_string()),
        (false, HostFreshness::Live) if source_stale => (StatusTheme::warn(), "STALE".to_string()),
        (false, HostFreshness::Live) => (StatusTheme::ok(), "LIVE".to_string()),
        (false, HostFreshness::Stale) => (
            StatusTheme::warn(),
            format!("STALE · {}", format_status_age(sample_age)),
        ),
        (false, HostFreshness::Offline) => (StatusTheme::bad(), "OFFLINE".to_string()),
        (false, HostFreshness::Down) => (StatusTheme::bad(), "DOWN".to_string()),
    };
    let border = match freshness {
        HostFreshness::Down | HostFreshness::Offline if !source_offline => {
            StatusTheme::bad().opacity(0.7)
        }
        HostFreshness::Stale if !source_offline => StatusTheme::warn().opacity(0.6),
        _ => StatusTheme::card_border(),
    };
    let dim = source_stale || source_offline || freshness != HostFreshness::Live;

    let title = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(s(10.))
                .child(
                    div()
                        .text_size(s(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(StatusTheme::text())
                        .child(host.name.clone()),
                )
                .child(
                    div()
                        .text_size(s(15.))
                        .text_color(StatusTheme::muted())
                        .child(host.role.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(s(8.))
                .px(s(10.))
                .py(s(3.))
                .rounded(s(999.))
                .bg(badge_color.opacity(0.14))
                .child(div().size(s(9.)).rounded(s(5.)).bg(badge_color))
                .child(
                    div()
                        .text_size(s(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(badge_color)
                        .child(badge_text),
                ),
        );

    let card = div()
        .flex()
        .flex_col()
        .gap(s(12.))
        .p(s(18.))
        .rounded(s(14.))
        .bg(StatusTheme::card())
        .border_1()
        .border_color(border)
        .min_h_0()
        .overflow_hidden()
        .child(title);

    if source_offline {
        return card.child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(s(18.))
                .text_color(StatusTheme::faint())
                .child("— no current data —"),
        );
    }
    if freshness == HostFreshness::Down || freshness == HostFreshness::Offline {
        let reason = if freshness == HostFreshness::Down {
            "Host unreachable"
        } else {
            "No recent sample"
        };
        return card.child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(s(6.))
                .child(
                    div()
                        .text_size(s(22.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(StatusTheme::bad())
                        .child(reason),
                )
                .child(
                    div()
                        .text_size(s(16.))
                        .text_color(StatusTheme::muted())
                        .child(format!("Last sample {} ago", format_status_age(sample_age))),
                ),
        );
    }

    let ratio = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f32 / total as f32),
        _ => None,
    };
    let usage = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) => format_status_usage(used, total),
        _ => "—".into(),
    };
    let temperature = host
        .temperature_celsius
        .map_or("—".into(), |t| format!("{t:.0} °C"));
    let temperature_color = match host.temperature_celsius {
        _ if dim => StatusTheme::muted(),
        Some(t) if t >= 85.0 => StatusTheme::bad(),
        Some(t) if t >= 75.0 => StatusTheme::warn(),
        _ => StatusTheme::text(),
    };
    let stat = |label: &str, value: String, color: Hsla| {
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
    };
    let value_color = if dim {
        StatusTheme::muted()
    } else {
        StatusTheme::text()
    };

    card.child(metric_bar(
        &s,
        "CPU",
        host.cpu_percent.map(|p| p / 100.0),
        host.cpu_percent.map_or("—".into(), |p| format!("{p:.0} %")),
        dim,
    ))
    .child(metric_bar(
        &s,
        "MEM",
        ratio(host.memory_used_bytes, host.memory_total_bytes),
        usage(host.memory_used_bytes, host.memory_total_bytes),
        dim,
    ))
    .child(metric_bar(
        &s,
        "DISK",
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
            .child(stat("TEMP", temperature, temperature_color))
            .child(stat(
                "LOAD",
                host.load_1m.map_or("—".into(), |l| format!("{l:.2}")),
                value_color,
            ))
            .child(stat(
                "NET",
                format!(
                    "↓ {}  ↑ {}",
                    host.network_rx_bytes_per_second
                        .map_or("—".into(), format_status_rate),
                    host.network_tx_bytes_per_second
                        .map_or("—".into(), format_status_rate)
                ),
                value_color,
            ))
            .child(stat(
                "UP",
                host.uptime_seconds.map_or("—".into(), format_status_uptime),
                value_color,
            )),
    )
}

fn service_pill(s: impl Fn(f32) -> Pixels, service: &ServiceStatus, dim: bool) -> Div {
    let color = match (dim, service.state) {
        (true, _) => StatusTheme::faint(),
        (false, ServiceState::Ok) => StatusTheme::ok(),
        (false, ServiceState::Degraded) => StatusTheme::warn(),
        (false, ServiceState::Down) => StatusTheme::bad(),
    };
    let text = match &service.detail {
        Some(detail) if service.state != ServiceState::Ok => format!("{} — {detail}", service.name),
        _ => service.name.clone(),
    };
    div()
        .flex()
        .items_center()
        .gap(s(8.))
        .px(s(12.))
        .py(s(5.))
        .rounded(s(999.))
        .bg(StatusTheme::card())
        .border_1()
        .border_color(if service.state == ServiceState::Ok || dim {
            StatusTheme::card_border()
        } else {
            color.opacity(0.6)
        })
        .child(div().size(s(10.)).rounded(s(5.)).bg(color))
        .child(
            div()
                .text_size(s(15.))
                .text_color(StatusTheme::text())
                .child(text),
        )
}
