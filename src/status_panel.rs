//! Feed panels: one status panel for each feed, drawn as its own GPUI view.
//!
//! Each panel is a cached view, as the Zed panes and docks are. GPUI reuses
//! the last drawing of a panel until the panel is notified: by a new poll
//! result of its feed, or by its redraw timer when its time-based text
//! changes.

use time::OffsetDateTime;

use crate::{
    status_board::{StatusBoard, StatusFeedUpdated},
    status_collector::StatusCollector,
    status_feed::{HostHistory, StatusFeed},
    status_format::{
        format_status_bytes, format_status_rate, format_status_uptime, format_status_usage,
    },
    status_freshness::FeedFreshness,
    status_graph::{NETWORK_GRAPH_FLOOR, cpu_graph, network_graph},
    status_model::{
        DiskHealth, DiskState, FeedData, HostMetrics, HostPresence, ServiceHealth, ServiceState,
        TailnetConnection, TailnetDevice,
    },
    status_redraw::StatusRedrawTimer,
    status_theme::{
        CORE_GRID_ROW_HEIGHT, HOST_PANEL_BASE_HEIGHT, Scale, StatusLayout, StatusTheme,
        threshold_color,
    },
};
use gpui::{
    AnyElement, Context, Div, Entity, FontWeight, Hsla, IntoElement, Render, Subscription, Window,
    div, prelude::*, relative,
};

/// Feed panel view: the status panel of one feed.
pub struct FeedPanelView {
    board: Entity<StatusBoard>,
    index: usize,
    /// Number of host panels that share the host column. More than one gives
    /// compact host panels.
    host_rows: usize,
    /// Devices with their own host card. The tailnet panel leaves them out.
    carded: Vec<String>,
    redraw: StatusRedrawTimer,
    _subscription: Subscription,
}

impl FeedPanelView {
    /// Creates the panel of the feed at `index` in the board.
    pub fn new_feed_panel(
        board: Entity<StatusBoard>,
        index: usize,
        host_rows: usize,
        carded: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&board, |this, _, event: &StatusFeedUpdated, cx| {
            if event.index == this.index {
                cx.notify();
            }
        });
        Self {
            board,
            index,
            host_rows,
            carded,
            redraw: StatusRedrawTimer::default(),
            _subscription: subscription,
        }
    }
}

impl Render for FeedPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = StatusLayout::from_status_window(window);
        let scale = move |value: f32| layout.scale(value);
        let s: Scale = &scale;
        let panel = {
            let board = self.board.read(cx);
            let feed = &board.status_feeds()[self.index];
            if let Some(change) = board.next_freshness_change(feed) {
                self.redraw.redraw_after(change);
            }
            let view = FeedView {
                feed,
                freshness: board.feed_freshness(feed),
                redraw: &self.redraw,
                now: OffsetDateTime::now_utc(),
            };
            let compact = self.host_rows > 1;
            match &feed.spec.collector {
                StatusCollector::Glances(_) | StatusCollector::Local(_) => {
                    host_panel(s, &view, layout.host_panel_height(self.host_rows), compact)
                }
                StatusCollector::TailnetHost(_) => presence_panel(s, &view, compact),
                StatusCollector::Kuma(_) => services_panel(s, &view),
                StatusCollector::Scrutiny(_) => disks_panel(s, &view),
                StatusCollector::Tailscale(_) => tailnet_panel(s, &view, &self.carded),
            }
        };
        self.redraw.schedule_status_redraw(cx);
        panel
    }
}

/// What a feed panel shows, with the feed's freshness applied.
struct FeedView<'a> {
    feed: &'a StatusFeed,
    freshness: FeedFreshness,
    /// Redraw timer of the panel. Age text asks it for a redraw.
    redraw: &'a StatusRedrawTimer,
    now: OffsetDateTime,
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

    /// Age of a wall-clock time, such as `4s ago`.
    fn ago(&self, at: Option<OffsetDateTime>) -> String {
        self.redraw.status_ago_text(self.now, at)
    }
}

/// Panel frame: title, one header fact or the freshness state, and body.
fn feed_panel(
    s: Scale,
    view: &FeedView,
    header_fact: Option<String>,
    body: AnyElement,
    compact: bool,
) -> Div {
    let (padding, gap) = if compact { (12., 8.) } else { (16., 12.) };
    let (state_text, state_color) = match view.freshness {
        FeedFreshness::Waiting => (Some("connecting".to_string()), StatusTheme::info()),
        FeedFreshness::Live => (header_fact, StatusTheme::muted()),
        FeedFreshness::Stale => (
            view.feed
                .last_success_age()
                .map(|age| format!("stale {}", view.redraw.status_age_text(age))),
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
        .size_full()
        .flex()
        .flex_col()
        .gap(s(gap))
        .p(s(padding))
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

fn host_panel(s: Scale, view: &FeedView, panel_height: f32, compact: bool) -> Div {
    let host = match view.reading() {
        Some(FeedData::Host(host)) => host,
        // Glances does not answer, and Tailscale knows the host.
        Some(FeedData::HostPresence(_)) => return presence_panel(s, view, compact),
        _ => return feed_panel(s, view, None, empty_body(s, view), compact),
    };
    let history = view.feed.host_history();
    let body = if compact {
        compact_host_body(s, host, history, view.dim()).into_any_element()
    } else {
        host_body(s, host, history, panel_height, view.dim()).into_any_element()
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
fn compact_host_body(s: Scale, host: &HostMetrics, history: &HostHistory, dim: bool) -> Div {
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
    let rate_color = |color: Hsla| if dim { StatusTheme::muted() } else { color };
    let meters = div()
        .w(relative(0.6))
        .flex_none()
        .flex()
        .flex_col()
        .gap(s(6.))
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
    let activity = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(s(6.))
        .child(cpu_graph(s, &history.cpu_percent, dim))
        .child(
            div()
                .flex()
                .justify_end()
                .gap(s(12.))
                .text_size(s(13.))
                .child(
                    div()
                        .text_color(rate_color(StatusTheme::receive()))
                        .child(format!("↓ {}", rate_text(host.network_receive_rate))),
                )
                .child(
                    div()
                        .text_color(rate_color(StatusTheme::send()))
                        .child(format!("↑ {}", rate_text(host.network_send_rate))),
                ),
        );
    // The body fills the card, so the CPU graph grows into the free height.
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .gap(s(20.))
        .child(meters)
        .child(activity)
}

fn compact_metric_row(
    s: Scale,
    label: &str,
    fraction: Option<f32>,
    value: String,
    dim: bool,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(10.))
        .text_size(s(14.))
        .child(
            div()
                .w(s(40.))
                .flex_none()
                .text_color(StatusTheme::muted())
                .child(label.to_string()),
        )
        .child(meter(s, fraction, 5., dim))
        .child(
            div()
                .w(s(160.))
                .flex_none()
                .flex()
                .justify_end()
                .text_color(value_color(dim))
                .child(value),
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

fn services_panel(s: Scale, view: &FeedView) -> Div {
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
                format!("{state} {}", view.ago(service.last_heartbeat)),
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
        false,
    )
}

fn disks_panel(s: Scale, view: &FeedView) -> Div {
    let Some(FeedData::Disks(disks)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view), false);
    };
    let dim = view.dim();
    let passed = disks
        .iter()
        .filter(|d| d.state == DiskState::Passed)
        .count();
    let rows = disks.iter().map(|disk: &DiskHealth| {
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
                (state.to_string(), 88., color),
            ],
        )
    });
    feed_panel(
        s,
        view,
        Some(format!("{passed}/{} passed", disks.len())),
        list_body(s, rows),
        false,
    )
}

fn tailnet_panel(s: Scale, view: &FeedView, carded: &[String]) -> Div {
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
    let online = devices.iter().filter(|d| d.online).count();
    let rows = devices.iter().map(|device| {
        let color = match (dim, device.online) {
            (true, _) | (_, false) => StatusTheme::faint(),
            (_, true) => StatusTheme::ok(),
        };
        let connection = tailnet_connection_text(view, device);
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
        false,
    )
}

/// Host card for a host without metrics now: its Tailscale presence.
fn presence_panel(s: Scale, view: &FeedView, compact: bool) -> Div {
    let Some(FeedData::HostPresence(HostPresence {
        device,
        metrics_error,
    })) = view.reading()
    else {
        return feed_panel(s, view, None, empty_body(s, view), compact);
    };
    let dim = view.dim();
    let (dot, state) = if device.online {
        (StatusTheme::ok(), "online".to_string())
    } else {
        let seen = device
            .last_seen
            .map(|_| format!("offline, seen {}", view.ago(device.last_seen)))
            .unwrap_or_else(|| "offline".into());
        (StatusTheme::faint(), seen)
    };
    let dot = if dim { StatusTheme::faint() } else { dot };
    let state_color = if device.online && !dim {
        StatusTheme::text()
    } else {
        StatusTheme::muted()
    };
    let state_row = div()
        .flex()
        .items_center()
        .gap(s(10.))
        .text_size(s(14.))
        .child(status_dot(s, dot))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(state_color)
                .child(state),
        );
    // An offline device shows its last-seen age in the body only.
    let path = device.online.then(|| tailnet_connection_text(view, device));
    let Some(error) = metrics_error else {
        // Status card: one row with the online state and the OS. It has the
        // fixed height `PRESENCE_PANEL_HEIGHT`.
        let row = state_row.child(
            div()
                .flex_none()
                .text_color(StatusTheme::muted())
                .child(device.os.clone()),
        );
        return feed_panel(s, view, path, row.into_any_element(), true);
    };
    let body = div()
        .flex()
        .flex_col()
        .gap(s(6.))
        .text_size(s(14.))
        .child(state_row)
        .child(
            div()
                .text_color(StatusTheme::muted())
                .child(device.os.clone()),
        )
        .child(presence_note(s, device, error, dim));
    feed_panel(s, view, path, body.into_any_element(), compact)
}

/// Note under the presence card of a Glances host: why the card shows no
/// metrics. A host that is offline is a normal state, so its note is gray.
/// An online host whose Glances does not answer gets an amber note with the
/// error.
fn presence_note(s: Scale, device: &TailnetDevice, metrics_error: &str, dim: bool) -> Div {
    let note = div().flex().flex_col().gap(s(2.)).text_size(s(13.));
    match metrics_error {
        _ if !device.online => note
            .text_color(StatusTheme::faint())
            .child("Metrics show again when the host is online."),
        error => note
            .child(
                div()
                    .text_color(if dim {
                        StatusTheme::muted()
                    } else {
                        StatusTheme::warn()
                    })
                    .child("Glances does not answer."),
            )
            .child(
                div()
                    .text_color(StatusTheme::faint())
                    .truncate()
                    .child(error.to_string()),
            ),
    }
}

/// Tailscale path to a device, such as `direct` or `relay lax`.
fn tailnet_connection_text(view: &FeedView, device: &TailnetDevice) -> String {
    match &device.connection {
        TailnetConnection::ThisMachine => "this machine".to_string(),
        TailnetConnection::Direct => "direct".to_string(),
        TailnetConnection::Relay(relay) => format!("relay {relay}"),
        TailnetConnection::Idle => "idle".to_string(),
        TailnetConnection::Offline => match device.last_seen {
            Some(_) => view.ago(device.last_seen),
            None => "offline".to_string(),
        },
    }
}
