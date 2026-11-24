//! Status view: the full-screen Rune layout of the status display. It
//! matches the v1 GPUI layout: wallpaper, clock, alerts, and the panels.

use std::time::Duration;

use rune::{Color, Cx, El, SurfaceFit, TextStyle, col, el, row, text};
use server_status_core::{
    status_collector::StatusCollector,
    status_feed::StatusFeed,
    status_format::{
        format_status_age, format_status_bytes, format_status_clock, format_status_date,
        format_status_rate, format_status_uptime, format_status_usage,
    },
    status_freshness::FeedFreshness,
    status_model::{
        DiskHealth, DiskState, FeedData, HostMetrics, ServiceHealth, ServiceState,
        TailnetConnection, TailnetDevice,
    },
    status_startup::{StartupMilestone, log_startup_milestone},
};
use time::OffsetDateTime;

use crate::{
    status_graphs::{
        NETWORK_GRAPH_FLOOR, core_grid_columns, core_usage_grid, cpu_history_graph,
        network_history_graph, status_meter,
    },
    status_state::StatusState,
    status_theme::{DESIGN_WIDTH, StatusScale, StatusTheme},
};

/// Layout key of the root element. The view reads its size for the scale.
const STATUS_ROOT_KEY: &str = "status-root";

/// Window size before the first layout, in points.
pub const STATUS_WINDOW_SIZE: (f32, f32) = (1600.0, 900.0);

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

/// One line of text in a style and a color.
fn label(content: impl Into<String>, style: TextStyle, color: Color) -> El {
    text(content).font(style).color(color)
}

/// `AppSpec::view`: the whole status display.
pub fn status_view(state: &StatusState, cx: &mut Cx<StatusState>) -> El {
    log_startup_milestone(StartupMilestone::FirstViewBuilt);
    let [width, height] = cx
        .layout_size(STATUS_ROOT_KEY)
        .unwrap_or([STATUS_WINDOW_SIZE.0, STATUS_WINDOW_SIZE.1]);
    let s = StatusScale::from_window_width(width);
    let feeds: Vec<FeedView> = state
        .feeds
        .iter()
        .map(|feed| FeedView {
            feed,
            freshness: state.feed_freshness(feed),
        })
        .collect();

    let content = col()
        .w_full()
        .h_full()
        .gap(s.px(24.0))
        .padding(s.px(32.0))
        .child(top_area(s, state, &feeds))
        .child(panel_area(s, state, &feeds, height / s.0));

    let mut root = el()
        .key(STATUS_ROOT_KEY)
        .w_full()
        .h_full()
        .fill(StatusTheme::background())
        .on_pointer_move_local(|state: &mut StatusState, _, tx| state.show_status_controls(tx));
    if let Some(wallpaper) = &state.wallpaper {
        root = root.child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .pixels_fit(wallpaper.pixels(), SurfaceFit::Fill),
        );
    }
    root.child(
        el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .fill(StatusTheme::scrim()),
    )
    .child(content)
}

/// Top area: the clock on the left; the controls and alerts on the right.
fn top_area(s: StatusScale, state: &StatusState, feeds: &[FeedView]) -> El {
    let now = state.now;
    let offset = state.local_offset;
    let clock = col()
        .child(label(
            format_status_clock(now, offset),
            s.sans(128.0, 300.0).lh(s.px(128.0)),
            StatusTheme::text(),
        ))
        .child(el().pl(s.px(6.0)).child(label(
            format_status_date(now, offset),
            s.sans(28.0, 400.0),
            StatusTheme::text(),
        )));

    let mut alerts = Vec::new();
    if state.window_focused {
        alerts.push(alert_row(
            s,
            StatusTheme::warn(),
            "This window has keyboard focus.".into(),
            "Moonlight input goes here, not to the game. Click the game to return it.".into(),
        ));
    }
    alerts.extend(feeds.iter().filter_map(|view| feed_alert(s, view)));

    let mut right = col().w_frac(0.5).items_end().gap(s.px(8.0));
    if state.controls_visible {
        right = right.child(window_controls(s, state));
    }
    row()
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .justify_between()
        .items_start()
        .gap(s.px(32.0))
        .child(clock)
        .child(right.children(alerts))
}

/// Window controls. They show for a few seconds after the pointer moves.
fn window_controls(s: StatusScale, state: &StatusState) -> El {
    let full_screen_label = if state.is_full_screen() {
        "Windowed"
    } else {
        "Fullscreen"
    };
    row()
        .items_center()
        .gap(s.px(8.0))
        .child(el().pr(s.px(8.0)).child(label(
            state.display_label.clone(),
            s.mono(13.0, 400.0),
            StatusTheme::muted(),
        )))
        .child(
            control_button(s, full_screen_label)
                .on_press_tx(|state: &mut StatusState, tx| state.toggle_full_screen(tx)),
        )
        .child(
            control_button(s, "Next display")
                .on_press(|state: &mut StatusState| state.next_status_display()),
        )
        .child(
            control_button(s, "Refresh").on_press(|state: &mut StatusState| state.refresh_status()),
        )
        .child(control_button(s, "Quit").on_press_tx(|_: &mut StatusState, tx| tx.quit()))
}

fn control_button(s: StatusScale, content: &'static str) -> El {
    el().px(s.px(12.0))
        .py(s.px(4.0))
        .radius(s.px(6.0))
        .fill(StatusTheme::panel())
        .border(s.px(1.0), StatusTheme::panel_border())
        .hover(Color::WHITE.alpha(0.08))
        .child(label(content, s.mono(13.0, 400.0), StatusTheme::muted()))
}

/// Alert for a feed that is stale or offline. Live feeds give none.
fn feed_alert(s: StatusScale, view: &FeedView) -> Option<El> {
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
                format_status_age(feed.policy.poll_interval)
            ),
        )),
        FeedFreshness::Waiting | FeedFreshness::Live => None,
    }
}

fn alert_row(s: StatusScale, color: Color, title: String, detail: String) -> El {
    col()
        .max_w(s.px(DESIGN_WIDTH / 2.0))
        .gap(s.px(2.0))
        .px(s.px(16.0))
        .py(s.px(8.0))
        .radius(s.px(8.0))
        .fill(StatusTheme::panel())
        .border(s.px(1.0), color.alpha(0.7))
        .child(label(title, s.mono(15.0, 600.0), color))
        .child(label(detail, s.mono(13.0, 400.0), StatusTheme::muted()).truncate())
}

/// Panel area: host columns, then the services column, then the disks and
/// tailnet column. `height` is the window height in design pixels.
fn panel_area(s: StatusScale, state: &StatusState, feeds: &[FeedView], height: f32) -> El {
    let now = state.now;
    let is_host = |view: &&FeedView| view.feed.spec.collector.is_host_collector();
    // Up to two hosts get one column each. More hosts share one column.
    let host_count = feeds.iter().filter(is_host).count();
    let host_rows = if host_count <= 2 { 1 } else { host_count };
    let panel_height = (height * PANEL_AREA_HEIGHT - PANEL_GAP * (host_rows as f32 - 1.0))
        / host_rows.max(1) as f32;
    let compact = host_rows > 1;
    // Devices with a host card do not repeat in the tailnet list.
    let carded: Vec<&str> = feeds
        .iter()
        .filter(is_host)
        .map(|view| view.feed.spec.title.as_str())
        .collect();

    let mut host_panels = Vec::new();
    let mut service_panels = Vec::new();
    let mut side_panels = Vec::new();
    for view in feeds {
        match &view.feed.spec.collector {
            StatusCollector::Glances(_) | StatusCollector::Local(_) => {
                host_panels.push(host_panel(s, view, panel_height, compact))
            }
            StatusCollector::TailnetHost(_) => {
                host_panels.push(presence_panel(s, view, now, compact))
            }
            StatusCollector::Kuma(_) => service_panels.push(services_panel(s, view, now)),
            StatusCollector::Scrutiny(_) => side_panels.push(disks_panel(s, view)),
            StatusCollector::Tailscale(_) => side_panels.push(tailnet_panel(s, view, now, &carded)),
        }
    }
    let column = |panels: Vec<El>, grow: f32| {
        (!panels.is_empty()).then(|| {
            col()
                .grow(grow)
                .min_w(0.0)
                .h_full()
                .gap(s.px(PANEL_GAP))
                .children(panels)
        })
    };
    let mut columns: Vec<El> = if host_rows == 1 {
        let grow = if host_count == 1 { 1.3 } else { 1.0 };
        host_panels
            .into_iter()
            .filter_map(|panel| column(vec![panel], grow))
            .collect()
    } else {
        column(host_panels, 1.3).into_iter().collect()
    };
    columns.extend(column(service_panels, 1.0));
    columns.extend(column(side_panels, 1.0));
    row()
        .w_full()
        .h(s.px(height * PANEL_AREA_HEIGHT))
        .gap(s.px(PANEL_GAP))
        .children(columns)
}

/// Panel frame: title, one header fact or the freshness state, and body.
fn feed_panel(
    s: StatusScale,
    view: &FeedView,
    header_fact: Option<String>,
    body: El,
    compact: bool,
) -> El {
    let (padding, gap) = if compact { (12.0, 8.0) } else { (16.0, 12.0) };
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
        FeedFreshness::Stale => StatusTheme::warn().alpha(0.7),
        FeedFreshness::Offline => StatusTheme::bad().alpha(0.7),
        _ => StatusTheme::panel_border(),
    };
    let mut header = row()
        .w_full()
        .items_end()
        .justify_between()
        .gap(s.px(12.0))
        .child(
            label(
                view.feed.spec.title.clone(),
                s.mono(17.0, 600.0),
                StatusTheme::text(),
            )
            .truncate()
            .shrink(),
        );
    if let Some(state_text) = state_text {
        header = header.child(label(state_text, s.mono(14.0, 400.0), state_color));
    }
    col()
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .gap(s.px(gap))
        .padding(s.px(padding))
        .radius(s.px(8.0))
        .fill(StatusTheme::panel())
        .border(s.px(1.0), border)
        .clip_rounded(s.px(8.0))
        .child(header)
        .child(body)
}

/// Body for a feed without data: waiting, or offline with the error.
fn empty_body(s: StatusScale, view: &FeedView) -> El {
    let message = match view.freshness {
        FeedFreshness::Offline => "No data",
        _ => "Waiting for data",
    };
    let mut body = col()
        .grow(1.0)
        .min_h(0.0)
        .justify_center()
        .gap(s.px(4.0))
        .child(label(message, s.mono(16.0, 400.0), StatusTheme::muted()));
    if let Some(error) = view.feed.last_poll_error() {
        body = body.child(label(error, s.mono(13.0, 400.0), StatusTheme::faint()).wrap());
    }
    body
}

fn value_color(dim: bool) -> Color {
    if dim {
        StatusTheme::muted()
    } else {
        StatusTheme::text()
    }
}

/// Right-aligned text in a fixed-width box.
fn right_aligned(width: f32, content: El) -> El {
    row().w(width).justify_end().child(content)
}

fn metric_row(s: StatusScale, name: &str, fraction: Option<f32>, value: String, dim: bool) -> El {
    row()
        .w_full()
        .items_center()
        .gap(s.px(12.0))
        .child(el().w(s.px(48.0)).child(label(
            name.to_string(),
            s.mono(14.0, 400.0),
            StatusTheme::muted(),
        )))
        .child(status_meter(s, fraction, 6.0, dim))
        .child(right_aligned(
            s.px(152.0),
            label(value, s.mono(14.0, 400.0), value_color(dim)),
        ))
}

fn host_panel(s: StatusScale, view: &FeedView, panel_height: f32, compact: bool) -> El {
    let Some(FeedData::Host(host)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view), compact);
    };
    let body = if compact {
        compact_host_body(s, host, view)
    } else {
        host_body(s, host, view, panel_height)
    };
    feed_panel(
        s,
        view,
        host.uptime_seconds
            .map(|up| format!("up {}", format_status_uptime(up))),
        body,
        compact,
    )
}

/// Compact host body for a shared host column: CPU, memory, and disk meters
/// on the left; the CPU graph and the network rates on the right.
fn compact_host_body(s: StatusScale, host: &HostMetrics, view: &FeedView) -> El {
    let dim = view.dim();
    let history = view.feed.host_history();
    let ratio = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f32 / total as f32),
        _ => None,
    };
    let usage = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) => format_status_usage(used, total),
        _ => "—".into(),
    };
    let mut cpu_value = host.cpu_percent.map_or("—".into(), |p| format!("{p:.0}%"));
    if let Some(t) = host.temperature_celsius {
        cpu_value = format!("{cpu_value}  {t:.0}°C");
    }
    let rate_text = |rate: Option<f64>| rate.map_or("—".into(), format_status_rate);
    let rate_color = |color: Color| if dim { StatusTheme::muted() } else { color };
    let meters = col()
        .w_frac(0.6)
        .gap(s.px(6.0))
        .child(compact_metric_row(
            s,
            "CPU",
            host.cpu_percent.map(|p| p / 100.0),
            cpu_value,
            dim,
        ))
        .child(compact_metric_row(
            s,
            "Mem",
            ratio(host.memory_used_bytes, host.memory_total_bytes),
            usage(host.memory_used_bytes, host.memory_total_bytes),
            dim,
        ))
        .child(compact_metric_row(
            s,
            "Disk",
            ratio(host.disk_used_bytes, host.disk_total_bytes),
            usage(host.disk_used_bytes, host.disk_total_bytes),
            dim,
        ));
    let activity = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(s.px(6.0))
        .child(cpu_history_graph(s, &history.cpu_percent, dim))
        .child(
            row()
                .gap(s.px(12.0))
                .justify_end()
                .child(label(
                    format!("↓ {}", rate_text(host.network_receive_rate)),
                    s.mono(13.0, 400.0),
                    rate_color(StatusTheme::receive()),
                ))
                .child(label(
                    format!("↑ {}", rate_text(host.network_send_rate)),
                    s.mono(13.0, 400.0),
                    rate_color(StatusTheme::send()),
                )),
        );
    row().w_full().gap(s.px(20.0)).child(meters).child(activity)
}

fn compact_metric_row(
    s: StatusScale,
    name: &str,
    fraction: Option<f32>,
    value: String,
    dim: bool,
) -> El {
    row()
        .w_full()
        .items_center()
        .gap(s.px(10.0))
        .child(el().w(s.px(40.0)).child(label(
            name.to_string(),
            s.mono(14.0, 400.0),
            StatusTheme::muted(),
        )))
        .child(status_meter(s, fraction, 5.0, dim))
        .child(right_aligned(
            s.px(160.0),
            label(value, s.mono(14.0, 400.0), value_color(dim)),
        ))
}

/// Host card for a host without a metrics server: its Tailscale presence.
fn presence_panel(s: StatusScale, view: &FeedView, now: OffsetDateTime, compact: bool) -> El {
    let Some(FeedData::TailnetHost(device)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view), compact);
    };
    let dim = view.dim();
    let (dot, state) = if device.online {
        (StatusTheme::ok(), "online".to_string())
    } else {
        let seen = device
            .last_seen
            .map(|_| format!("offline, seen {}", age_since(now, device.last_seen)))
            .unwrap_or_else(|| "offline".into());
        (StatusTheme::faint(), seen)
    };
    let dot = if dim { StatusTheme::faint() } else { dot };
    let state_color = if device.online && !dim {
        StatusTheme::text()
    } else {
        StatusTheme::muted()
    };
    let body = col()
        .w_full()
        .gap(s.px(6.0))
        .child(
            row()
                .items_center()
                .gap(s.px(10.0))
                .child(status_dot(s, dot))
                .child(label(state, s.mono(14.0, 400.0), state_color)),
        )
        .child(label(
            device.os.clone(),
            s.mono(14.0, 400.0),
            StatusTheme::muted(),
        ))
        .child(label(
            "No metrics server. Tailscale presence only.",
            s.mono(13.0, 400.0),
            StatusTheme::faint(),
        ));
    // An offline device shows its last-seen age in the body only.
    let path = device.online.then(|| tailnet_connection_text(device, now));
    feed_panel(s, view, path, body, compact)
}

/// Tailscale path to a device, such as `direct` or `relay lax`.
fn tailnet_connection_text(device: &TailnetDevice, now: OffsetDateTime) -> String {
    match &device.connection {
        TailnetConnection::ThisMachine => "this machine".to_string(),
        TailnetConnection::Direct => "direct".to_string(),
        TailnetConnection::Relay(relay) => format!("relay {relay}"),
        TailnetConnection::Idle => "idle".to_string(),
        TailnetConnection::Offline => match device.last_seen {
            Some(_) => age_since(now, device.last_seen),
            None => "offline".to_string(),
        },
    }
}

fn host_body(s: StatusScale, host: &HostMetrics, view: &FeedView, panel_height: f32) -> El {
    let dim = view.dim();
    let history = view.feed.host_history();
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
    let rate_color = |color: Color| if dim { StatusTheme::muted() } else { color };

    let mut cpu_header = row()
        .w_full()
        .items_end()
        .gap(s.px(16.0))
        .child(label("CPU", s.mono(15.0, 400.0), StatusTheme::muted()))
        .child(label(
            host.cpu_percent.map_or("—".into(), |p| format!("{p:.0}%")),
            s.mono(28.0, 400.0).lh(s.px(30.0)),
            value_color(dim),
        ))
        .child(el().grow(1.0));
    if let Some(t) = host.temperature_celsius {
        cpu_header = cpu_header.child(label(
            format!("{t:.0}°C"),
            s.mono(15.0, 400.0),
            temperature_color,
        ));
    }
    if let Some(load) = host.load_1m {
        cpu_header = cpu_header.child(label(
            format!("load {load:.2}"),
            s.mono(15.0, 400.0),
            StatusTheme::muted(),
        ));
    }
    let network_header = row()
        .w_full()
        .items_end()
        .gap(s.px(16.0))
        .child(label("Net", s.mono(15.0, 400.0), StatusTheme::muted()))
        .child(label(
            format!("↓ {}", rate_text(host.network_receive_rate)),
            s.mono(15.0, 400.0),
            rate_color(StatusTheme::receive()),
        ))
        .child(label(
            format!("↑ {}", rate_text(host.network_send_rate)),
            s.mono(15.0, 400.0),
            rate_color(StatusTheme::send()),
        ));

    let mut body = col()
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .gap(s.px(12.0))
        .child(cpu_header)
        .child(cpu_history_graph(s, &history.cpu_percent, dim));
    if show_cores {
        body = body.child(core_usage_grid(s, &host.core_percents, dim));
    }
    body.child(network_header)
        .child(network_history_graph(s, history, peak, dim))
        .child(
            col()
                .w_full()
                .gap(s.px(8.0))
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

fn status_dot(s: StatusScale, color: Color) -> El {
    el().size(s.px(8.0), s.px(8.0))
        .radius(s.px(4.0))
        .fill(color)
}

/// List row: state dot, name, then right-aligned columns of fixed width.
fn list_row(
    s: StatusScale,
    color: Color,
    name: String,
    name_color: Color,
    columns: Vec<(String, f32, Color)>,
) -> El {
    row()
        .w_full()
        .items_center()
        .gap(s.px(12.0))
        .child(status_dot(s, color))
        .child(
            el().grow(1.0).min_w(0.0).child(
                label(name, s.mono(14.0, 400.0), name_color)
                    .truncate()
                    .shrink(),
            ),
        )
        .children(columns.into_iter().map(|(content, width, color)| {
            right_aligned(s.px(width), label(content, s.mono(14.0, 400.0), color))
        }))
}

fn list_body(s: StatusScale, rows: impl IntoIterator<Item = El>) -> El {
    col().w_full().gap(s.px(8.0)).children(rows)
}

fn age_since(now: OffsetDateTime, at: Option<OffsetDateTime>) -> String {
    at.map(|at| {
        let age = Duration::try_from(now - at).unwrap_or_default();
        format!("{} ago", format_status_age(age))
    })
    .unwrap_or_else(|| "—".into())
}

fn services_panel(s: StatusScale, view: &FeedView, now: OffsetDateTime) -> El {
    let Some(FeedData::Services(services)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view), false);
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
        .filter(|service| service.state == ServiceState::Up)
        .count();
    let total = services.len();
    let dim = view.dim();
    let rows: Vec<El> = services
        .into_iter()
        .map(|service| {
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
                    176.0,
                    color,
                ));
            }
            columns.push((uptime, 64.0, StatusTheme::muted()));
            list_row(s, color, service.name.clone(), StatusTheme::text(), columns)
        })
        .collect();
    feed_panel(
        s,
        view,
        Some(format!("{up}/{total} up")),
        list_body(s, rows),
        false,
    )
}

fn disks_panel(s: StatusScale, view: &FeedView) -> El {
    let Some(FeedData::Disks(disks)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view), false);
    };
    let dim = view.dim();
    let passed = disks
        .iter()
        .filter(|disk| disk.state == DiskState::Passed)
        .count();
    let rows: Vec<El> = disks
        .iter()
        .map(|disk: &DiskHealth| {
            let (color, state) = match disk.state {
                DiskState::Passed => (StatusTheme::ok(), ""),
                DiskState::FailedSmart => (StatusTheme::bad(), "SMART failed"),
                DiskState::FailedScrutiny => (StatusTheme::warn(), "threshold"),
                DiskState::FailedBoth => (StatusTheme::bad(), "failed"),
            };
            let color = if dim { StatusTheme::faint() } else { color };
            list_row(
                s,
                color,
                disk.device.clone(),
                StatusTheme::text(),
                vec![
                    (
                        disk.temperature_celsius
                            .map_or(String::new(), |t| format!("{t:.0}°C")),
                        48.0,
                        StatusTheme::muted(),
                    ),
                    (
                        format_status_bytes(disk.capacity_bytes),
                        72.0,
                        StatusTheme::muted(),
                    ),
                    (state.to_string(), 88.0, color),
                ],
            )
        })
        .collect();
    feed_panel(
        s,
        view,
        Some(format!("{passed}/{} passed", disks.len())),
        list_body(s, rows),
        false,
    )
}

fn tailnet_panel(s: StatusScale, view: &FeedView, now: OffsetDateTime, carded: &[&str]) -> El {
    let Some(FeedData::Tailnet(devices)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view), false);
    };
    let devices: Vec<&TailnetDevice> = devices
        .iter()
        .filter(|device| {
            !carded
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&device.name))
        })
        .collect();
    let dim = view.dim();
    let online = devices.iter().filter(|device| device.online).count();
    let rows: Vec<El> = devices
        .iter()
        .map(|device| {
            let color = match (dim, device.online) {
                (true, _) | (_, false) => StatusTheme::faint(),
                (_, true) => StatusTheme::ok(),
            };
            let connection = tailnet_connection_text(device, now);
            let name_color = if device.online {
                StatusTheme::text()
            } else {
                StatusTheme::muted()
            };
            list_row(
                s,
                color,
                device.name.clone(),
                name_color,
                vec![
                    (device.os.clone(), 64.0, StatusTheme::muted()),
                    (connection, 104.0, StatusTheme::muted()),
                ],
            )
        })
        .collect();
    feed_panel(
        s,
        view,
        Some(format!("{online}/{} online", devices.len())),
        list_body(s, rows),
        false,
    )
}
