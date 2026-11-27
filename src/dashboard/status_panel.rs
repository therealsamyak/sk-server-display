//! Feed panels: one status panel for each feed, drawn as its own GPUI view.
//!
//! Each panel is a cached view, as the Zed panes and docks are. GPUI reuses
//! the last drawing of a panel until the panel is notified: by a new poll
//! result of its feed, or by its redraw timer when its time-based text
//! changes.

use time::OffsetDateTime;

use crate::{
    dashboard::status_format::{
        format_status_bytes, format_status_rate, format_status_uptime, format_status_usage,
    },
    dashboard::status_graph::{cpu_graph, network_graph},
    dashboard::status_redraw::StatusRedrawTimer,
    dashboard::status_service_history::{
        service_history_strip, service_state_color, service_summary_strip,
    },
    dashboard::status_theme::{
        HOST_CARD_HEIGHT, Scale, StatusLayout, StatusTheme, threshold_color,
    },
    metrics::FeedFreshness,
    metrics::StatusCollector,
    metrics::{
        DiskHealth, DiskState, FeedData, HostMetrics, HostPresence, ServiceHealth, ServiceState,
        TailnetConnection, TailnetDevice,
    },
    metrics::{HostHistory, StatusFeed},
    metrics::{StatusBoard, StatusFeedUpdated},
    platform::{BatteryLabelChanged, BatteryReading, LocalBatteryStatus},
    wallpaper::{StatusStyle, StyleArrangement, plain_status_style},
};
use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, EventEmitter, FontWeight, Hsla, IntoElement,
    Render, Role, ScrollHandle, Subscription, WeakEntity, Window, div, prelude::*, relative,
};

mod status_tailnet_rows;
#[cfg(test)]
mod status_tailnet_tests;

pub(super) use status_tailnet_rows::CardedTailnetDevices;

/// Feed panel view: the status panel of one feed.
pub struct FeedPanelView {
    board: Entity<StatusBoard>,
    index: usize,
    detail_scroll: Option<ScrollHandle>,
    style: StatusStyle,
    /// Devices with their own host card. The tailnet panel leaves them out.
    carded: CardedTailnetDevices,
    last_live_summary: Option<HostSummary>,
    battery: Option<Entity<LocalBatteryStatus>>,
    redraw: StatusRedrawTimer,
    _subscription: Subscription,
    _battery_subscription: Option<Subscription>,
}

impl FeedPanelView {
    /// Creates the panel of the feed at `index` in the board.
    pub fn new_feed_panel(
        board: Entity<StatusBoard>,
        index: usize,
        carded: CardedTailnetDevices,
        battery: Option<Entity<LocalBatteryStatus>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&board, |this, board, event: &StatusFeedUpdated, cx| {
            if event.index != this.index {
                return;
            }
            let board = board.read(cx);
            let feed = &board.status_feeds()[this.index];
            if let Some(shown) = &this.last_live_summary
                && board.feed_freshness(feed) == FeedFreshness::Live
                && let Some(FeedData::Host(host)) = feed.latest_reading()
                && *shown == HostSummary::from_metrics(host)
            {
                return;
            }
            cx.notify();
        });
        let battery_subscription = battery
            .as_ref()
            .map(|battery| cx.subscribe(battery, |_, _, _: &BatteryLabelChanged, cx| cx.notify()));
        Self {
            board,
            index,
            detail_scroll: None,
            style: plain_status_style(),
            carded,
            last_live_summary: None,
            battery,
            redraw: StatusRedrawTimer::default(),
            _subscription: subscription,
            _battery_subscription: battery_subscription,
        }
    }

    /// Set the style that belongs to the wallpaper shown in this window.
    pub fn set_panel_style(&mut self, style: StatusStyle, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    /// Create an expanded view of a host or service feed.
    pub fn new_expanded_panel(
        board: Entity<StatusBoard>,
        index: usize,
        style: StatusStyle,
        battery: Option<Entity<LocalBatteryStatus>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panel =
            Self::new_feed_panel(board, index, CardedTailnetDevices::default(), None, cx);
        panel.detail_scroll = Some(ScrollHandle::new());
        panel.style = style;
        panel._battery_subscription = battery
            .as_ref()
            .map(|battery| cx.observe(battery, |_, _, cx| cx.notify()));
        panel.battery = battery;
        panel
    }

    /// Draws the expanded card on the incoming page with its current scroll offset.
    pub fn new_incoming_detail(
        source: &Entity<Self>,
        style: StatusStyle,
        cx: &mut Context<Self>,
    ) -> Self {
        let source = source.read(cx);
        let offset = source.detail_scroll.as_ref().map(ScrollHandle::offset);
        let panel = Self::new_expanded_panel(
            source.board.clone(),
            source.index,
            style,
            source.battery.clone(),
            cx,
        );
        if let Some((offset, incoming)) = offset.zip(panel.detail_scroll.as_ref()) {
            incoming.set_offset(offset);
        }
        panel
    }
}

/// The user closed the expanded feed card.
pub struct FeedDetailsClosed;

impl EventEmitter<FeedDetailsClosed> for FeedPanelView {}

/// The user asked for the btop window of the expanded host card.
pub struct FeedBtopRequested;

impl EventEmitter<FeedBtopRequested> for FeedPanelView {}

impl Render for FeedPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = StatusLayout::from_status_window(window);
        let scale = move |value: f32| layout.scale(value);
        let s: Scale = &scale;
        let expanded = self.detail_scroll.is_some();
        let close_details = expanded.then(|| cx.weak_entity());
        let panel = {
            let board = self.board.read(cx);
            let feed = &board.status_feeds()[self.index];
            if let Some(change) = board.next_freshness_change(feed) {
                self.redraw.redraw_after(change);
            }
            let view = FeedView {
                feed,
                style: self.style,
                freshness: board.feed_freshness(feed),
                redraw: &self.redraw,
                now: OffsetDateTime::now_utc(),
                close_details,
                detail_scroll: self.detail_scroll.as_ref(),
            };
            self.last_live_summary = match view.reading() {
                Some(FeedData::Host(host))
                    if !expanded && view.freshness == FeedFreshness::Live =>
                {
                    Some(HostSummary::from_metrics(host))
                }
                _ => None,
            };
            let compact =
                expanded || matches!(self.style.arrangement, StyleArrangement::Sides { .. });
            match &feed.spec.collector {
                StatusCollector::Glances(_) | StatusCollector::Local(_) => {
                    let battery = self
                        .battery
                        .as_ref()
                        .map(|battery| battery.read(cx).battery_reading());
                    host_panel(s, &view, battery)
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

/// Line height of panel text, as a part of the text size.
pub const PANEL_LINE_HEIGHT: f32 = 1.25;

/// Height of the card title row. Header buttons match it, so a button
/// does not make the row taller than its title text.
const TITLE_ROW_HEIGHT: f32 = 17. * PANEL_LINE_HEIGHT;

/// Height of the collapsed Services header, including the inline history strip.
const SERVICE_SUMMARY_HEIGHT: f32 = 56.;
const HOST_CPU_HISTORY_HEIGHT: f32 = 40.;
const HOST_NETWORK_HISTORY_HEIGHT: f32 = 48.;
const HOST_DETAIL_BASE_HEIGHT: f32 = 192. + HOST_CPU_HISTORY_HEIGHT + HOST_NETWORK_HISTORY_HEIGHT;
const HOST_BATTERY_HEIGHT: f32 = 32.;
const CORE_GRID_ROW_HEIGHT: f32 = 23.;

/// Height of an expanded feed card within its column.
pub fn expanded_panel_height(feed: &StatusFeed, layout: StatusLayout) -> f32 {
    let height = match (&feed.spec.collector, feed.latest_reading()) {
        (StatusCollector::Kuma(_), Some(FeedData::Services(services))) => {
            list_panel_height(services.len())
        }
        (StatusCollector::Kuma(_), _) => 144.,
        (
            collector @ (StatusCollector::Local(_) | StatusCollector::Glances(_)),
            Some(FeedData::Host(host)),
        ) => {
            let core_rows = host
                .core_percents
                .len()
                .div_ceil(core_grid_columns(&host.core_percents));
            let core_height = if core_rows == 0 {
                0.
            } else {
                8. + CORE_GRID_ROW_HEIGHT * core_rows as f32
            };
            let battery_height = if matches!(collector, StatusCollector::Local(_)) {
                HOST_BATTERY_HEIGHT
            } else {
                0.
            };
            HOST_DETAIL_BASE_HEIGHT + battery_height + core_height
        }
        _ => 368.,
    };
    height.min(layout.panel_area)
}

/// Height of a list panel with `rows` rows, in design pixels: border,
/// padding, title row, gap, rows, and the gaps between rows.
fn list_panel_height(rows: usize) -> f32 {
    let title = 17. * PANEL_LINE_HEIGHT;
    let body = if rows == 0 {
        40.
    } else {
        rows as f32 * 14. * PANEL_LINE_HEIGHT + (rows - 1) as f32 * 8.
    };
    2. + 2. * 16. + title + 12. + body + 4.
}

/// Height of the panel of `feed` in a side column, in design pixels.
/// `carded` lists the devices that the tailnet panel leaves out.
pub fn side_panel_height(feed: &StatusFeed, carded: &CardedTailnetDevices) -> f32 {
    let rows = match (&feed.spec.collector, feed.latest_reading()) {
        (StatusCollector::Glances(_) | StatusCollector::Local(_), _) => {
            return HOST_CARD_HEIGHT;
        }
        (StatusCollector::TailnetHost(_), _) => return HOST_CARD_HEIGHT,
        (StatusCollector::Kuma(_), _) => return SERVICE_SUMMARY_HEIGHT,
        (_, Some(FeedData::Disks(disks))) => disks.len(),
        (_, Some(FeedData::Tailnet(devices))) => carded.visible_devices(devices).count(),
        _ => 0,
    };
    list_panel_height(rows)
}

/// What a feed panel shows, with the feed's freshness applied.
struct FeedView<'a> {
    feed: &'a StatusFeed,
    style: StatusStyle,
    freshness: FeedFreshness,
    /// Redraw timer of the panel. Age text asks it for a redraw.
    redraw: &'a StatusRedrawTimer,
    now: OffsetDateTime,
    close_details: Option<WeakEntity<FeedPanelView>>,
    detail_scroll: Option<&'a ScrollHandle>,
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
    let theme = StatusTheme(view.style.palette);
    let (padding, gap) = if compact { (12., 8.) } else { (16., 12.) };
    let (state_text, state_color) = match view.freshness {
        FeedFreshness::Waiting => (Some("connecting".to_string()), theme.info()),
        FeedFreshness::Live => (header_fact, theme.muted()),
        FeedFreshness::Stale => (
            view.feed
                .last_success_age()
                .map(|age| format!("stale {}", view.redraw.status_age_text(age))),
            theme.warn(),
        ),
        FeedFreshness::Offline => (Some("offline".to_string()), theme.bad()),
    };
    let kind = view.style.kind;
    let border = match view.freshness {
        FeedFreshness::Stale => theme.warn().opacity(0.7),
        FeedFreshness::Offline => theme.bad().opacity(0.7),
        _ => theme.panel_border(),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(s(gap))
        .p(s(padding))
        .rounded(s(kind.radius))
        .bg(if view.close_details.is_some() {
            Hsla {
                a: 1.,
                ..theme.panel()
            }
        } else {
            theme.panel()
        })
        .border_1()
        .border_color(border)
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_center()
                .gap(s(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(kind.title_family)
                        .text_size(s(17.))
                        .when(kind.title_semibold, |title| {
                            title.font_weight(FontWeight::SEMIBOLD)
                        })
                        .truncate()
                        .child(view.feed.spec.title.clone()),
                )
                .when(
                    view.close_details.is_some()
                        && !matches!(&view.feed.spec.collector, StatusCollector::Kuma(_)),
                    |row| {
                        row.child(
                            div()
                                .flex_none()
                                .text_size(s(13.))
                                .text_color(theme.muted())
                                .child(view.feed.spec.source),
                        )
                    },
                )
                .children(match view.reading() {
                    Some(FeedData::Services(services)) if compact && !services.is_empty() => {
                        Some(service_summary_strip(s, theme, services, view.dim()))
                    }
                    _ => None,
                })
                .children(state_text.map(|text| {
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(s(14.))
                        .text_color(state_color)
                        .child(text)
                }))
                .children(
                    view.close_details
                        .clone()
                        .filter(|_| {
                            view.feed.spec.collector.shows_host_btop()
                                && matches!(view.reading(), Some(FeedData::Host(_)))
                        })
                        .map(|panel| {
                            detail_button(
                                s,
                                theme,
                                kind.radius,
                                "open-btop",
                                "btop",
                                move |_, _, cx| {
                                    if let Err(error) =
                                        panel.update(cx, |_, cx| cx.emit(FeedBtopRequested))
                                    {
                                        log::debug!("feed details already closed: {error:#}");
                                    }
                                },
                            )
                        }),
                )
                .children(view.close_details.clone().map(|panel| {
                    div()
                        .id("collapse-feed-card")
                        .role(Role::Button)
                        .flex_none()
                        .h(s(TITLE_ROW_HEIGHT))
                        .px(s(8.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(s(13.))
                        .cursor_pointer()
                        .hover(|button| button.underline())
                        .on_click(move |_, _, cx| {
                            if let Err(error) = panel.update(cx, |_, cx| cx.emit(FeedDetailsClosed))
                            {
                                log::debug!("feed details already closed: {error:#}");
                            }
                        })
                        .child("Done")
                })),
        )
        .child(body)
}

fn detail_button(
    s: Scale,
    theme: StatusTheme,
    radius: f32,
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .role(Role::Button)
        .flex_none()
        .h(s(TITLE_ROW_HEIGHT))
        .px(s(8.))
        .flex()
        .items_center()
        .rounded(s(radius))
        .border_1()
        .border_color(theme.panel_border())
        .text_size(s(13.))
        .cursor_pointer()
        .hover(move |button| button.bg(theme.track()))
        .on_click(on_click)
        .child(label)
}

/// Body for a feed without data: waiting, or offline with the error.
fn empty_body(s: Scale, view: &FeedView) -> AnyElement {
    let theme = StatusTheme(view.style.palette);
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
                .text_color(theme.muted())
                .child(text),
        )
        .children(view.feed.last_poll_error().map(|error| {
            div()
                .text_size(s(13.))
                .text_color(theme.faint())
                .child(error.to_string())
        }))
        .into_any_element()
}

fn value_color(theme: StatusTheme, dim: bool) -> Hsla {
    if dim { theme.muted() } else { theme.text() }
}

fn meter(theme: StatusTheme, s: Scale, fraction: Option<f32>, height: f32, dim: bool) -> Div {
    let fraction = fraction.map(|f| f.clamp(0.0, 1.0));
    let color = match (fraction, dim) {
        (_, true) | (None, _) => theme.faint(),
        (Some(fraction), false) => threshold_color(theme, fraction),
    };
    div()
        .flex_1()
        .h(s(height))
        .rounded(s(height / 2.))
        .bg(theme.track())
        .child(
            div()
                .h_full()
                .rounded(s(height / 2.))
                .w(relative(fraction.unwrap_or(0.0)))
                .bg(color),
        )
}

fn metric_row(
    theme: StatusTheme,
    s: Scale,
    label: &str,
    fraction: Option<f32>,
    value: String,
    dim: bool,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(12.))
        .text_size(s(14.))
        .child(
            div()
                .w(s(48.))
                .flex_none()
                .text_color(theme.muted())
                .child(label.to_string()),
        )
        .child(meter(theme, s, fraction, 6., dim))
        .child(
            div()
                .w(s(152.))
                .flex_none()
                .flex()
                .justify_end()
                .text_color(value_color(theme, dim))
                .child(value),
        )
}

/// Per-core usage in a grid of small meters.
fn core_grid_columns(cores: &[f32]) -> usize {
    if cores.len() > 8 { 4 } else { 2 }
}

fn core_grid(theme: StatusTheme, s: Scale, cores: &[f32], dim: bool) -> Div {
    div()
        .grid()
        .grid_cols(core_grid_columns(cores) as u16)
        .gap_x(s(12.))
        .gap_y(s(6.))
        .text_size(s(13.))
        .children(cores.iter().enumerate().map(|(index, percent)| {
            div()
                .flex()
                .items_center()
                .gap(s(4.))
                .child(
                    div()
                        .w(s(16.))
                        .flex_none()
                        .text_color(theme.faint())
                        .child(index.to_string()),
                )
                .child(meter(theme, s, Some(percent / 100.0), 4., dim))
                .child(
                    div()
                        .w(s(34.))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .text_color(theme.muted())
                        .child(format!("{percent:.0}%")),
                )
        }))
}

fn host_panel(s: Scale, view: &FeedView, battery: Option<Option<&BatteryReading>>) -> Div {
    let detail = view.detail_scroll.is_some();
    let theme = StatusTheme(view.style.palette);
    let battery_label = (!detail)
        .then_some(battery.flatten())
        .flatten()
        .map(|reading| host_battery_label(s, theme, reading));
    let host = match view.reading() {
        Some(FeedData::Host(host)) => host,
        Some(FeedData::HostPresence(_)) => return presence_panel(s, view, !detail),
        _ => {
            let has_battery_label = battery_label.is_some();
            let body = if detail {
                empty_body(s, view)
            } else {
                div().children(battery_label).into_any_element()
            };
            return feed_panel(s, view, None, body, !detail)
                .when(!detail && !has_battery_label, |panel| {
                    panel.justify_center().gap_0()
                });
        }
    };
    if let Some(scroll) = view.detail_scroll {
        let body = host_body(
            theme,
            s,
            host,
            view.feed.host_history(),
            view.dim(),
            battery,
            scroll,
        );
        return feed_panel(s, view, None, body.into_any_element(), false);
    }
    let HostSummary { uptime, values } = HostSummary::from_metrics(host);
    let status = if battery.is_some() {
        battery_label
    } else {
        (view.freshness == FeedFreshness::Live).then(|| {
            div()
                .flex()
                .items_center()
                .gap(s(10.))
                .text_color(theme.ok())
                .child(status_dot(s, theme.ok()))
                .child("online")
        })
    };
    let body = host_summary(theme, s, status, values, view.dim());
    feed_panel(s, view, uptime, body.into_any_element(), true)
}

fn host_battery_label(s: Scale, theme: StatusTheme, reading: &BatteryReading) -> Div {
    div()
        .truncate()
        .text_size(s(13.))
        .text_color(theme.text())
        .child(reading.label.clone())
}

fn battery_details(s: Scale, theme: StatusTheme, reading: Option<&BatteryReading>) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x(s(12.))
        .text_size(s(13.))
        .child(div().text_color(theme.muted()).child("Battery"))
        .child(reading.map_or_else(|| "Unavailable".into(), |battery| battery.label.clone()))
        .children(reading.and_then(|battery| battery.net_watts).map(|watts| {
            div()
                .whitespace_nowrap()
                .child(format!("Net {watts:+.1} W"))
        }))
        .children(reading.filter(|battery| battery.on_battery).map(|battery| {
            let estimate = battery.remaining_seconds.map_or_else(
                || "Estimate unavailable".into(),
                |seconds| {
                    if seconds < 60 {
                        "<1m left".into()
                    } else {
                        format!("{} left", format_status_uptime(seconds))
                    }
                },
            );
            div().whitespace_nowrap().child(estimate)
        }))
}

/// Text shown by the compact host card, independent of its full metric history.
#[derive(PartialEq, Eq)]
struct HostSummary {
    uptime: Option<String>,
    values: [String; 3],
}

impl HostSummary {
    fn from_metrics(host: &HostMetrics) -> Self {
        let percent = |part: Option<u64>, total: Option<u64>| {
            part.zip(total).filter(|(_, total)| *total > 0).map_or_else(
                || "—".into(),
                |(part, total)| format!("{:.0}%", part as f64 / total as f64 * 100.),
            )
        };
        Self {
            uptime: host
                .uptime_seconds
                .map(|up| format!("up {}", format_status_uptime(up))),
            values: [
                host.cpu_percent
                    .map_or_else(|| "—".into(), |value| format!("{value:.0}%")),
                percent(host.memory_used_bytes, host.memory_total_bytes),
                percent(host.disk_used_bytes, host.disk_total_bytes),
            ],
        }
    }
}

fn host_summary(
    theme: StatusTheme,
    s: Scale,
    status: Option<Div>,
    values: [String; 3],
    dim: bool,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(8.))
        .text_size(s(13.))
        .child(div().flex_1().min_w_0().children(status))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(s(8.))
                .whitespace_nowrap()
                .children(
                    ["CPU", "Mem", "Disk"]
                        .into_iter()
                        .zip(values)
                        .map(|(label, value)| {
                            div()
                                .flex()
                                .items_center()
                                .gap(s(4.))
                                .child(div().text_color(theme.muted()).child(label))
                                .child(div().text_color(value_color(theme, dim)).child(value))
                        }),
                ),
        )
}

fn host_body(
    theme: StatusTheme,
    s: Scale,
    host: &HostMetrics,
    history: &HostHistory,
    dim: bool,
    battery: Option<Option<&BatteryReading>>,
    scroll: &ScrollHandle,
) -> impl IntoElement {
    let ratio = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) if total > 0 => Some(used as f32 / total as f32),
        _ => None,
    };
    let usage = |used: Option<u64>, total: Option<u64>| match (used, total) {
        (Some(used), Some(total)) => format_status_usage(used, total),
        _ => "—".into(),
    };
    let temperature_color = match host.temperature_celsius {
        _ if dim => theme.muted(),
        Some(t) if t >= 85.0 => theme.bad(),
        Some(t) if t >= 75.0 => theme.warn(),
        _ => theme.text(),
    };
    let rate_text = |rate: Option<f64>| rate.map_or("—".into(), format_status_rate);
    let network_header = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x(s(8.))
        .text_size(s(13.))
        .child(div().text_color(theme.muted()).child("Net"))
        .child(
            div()
                .text_color(if dim { theme.muted() } else { theme.receive() })
                .child(format!("↓ {}", rate_text(host.network_receive_rate))),
        )
        .child(
            div()
                .text_color(if dim { theme.muted() } else { theme.send() })
                .child(format!("↑ {}", rate_text(host.network_send_rate))),
        );
    let cpu_header = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(s(8.))
        .text_size(s(13.))
        .child(div().text_color(theme.muted()).child("CPU"))
        .child(
            div()
                .text_color(value_color(theme, dim))
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
                .text_color(theme.muted())
                .child(format!("load {load:.2}"))
        }));
    div()
        .id("host-detail-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .flex()
        .flex_col()
        .gap(s(8.))
        .child(cpu_header)
        .child(
            div()
                .h(s(HOST_CPU_HISTORY_HEIGHT))
                .flex_none()
                .flex()
                .flex_col()
                .child(cpu_graph(s, theme, &history.cpu_percent, dim)),
        )
        .child(network_header)
        .child(
            div()
                .h(s(HOST_NETWORK_HISTORY_HEIGHT))
                .flex_none()
                .flex()
                .flex_col()
                .child(network_graph(s, theme, history, dim)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(s(8.))
                .child(metric_row(
                    theme,
                    s,
                    "Mem",
                    ratio(host.memory_used_bytes, host.memory_total_bytes),
                    usage(host.memory_used_bytes, host.memory_total_bytes),
                    dim,
                ))
                .child(metric_row(
                    theme,
                    s,
                    "Disk",
                    ratio(host.disk_used_bytes, host.disk_total_bytes),
                    usage(host.disk_used_bytes, host.disk_total_bytes),
                    dim,
                )),
        )
        .children(battery.map(|reading| battery_details(s, theme, reading)))
        .when(!host.core_percents.is_empty(), |body| {
            body.child(core_grid(theme, s, &host.core_percents, dim))
        })
}

fn status_dot(s: Scale, color: Hsla) -> Div {
    div().flex_none().size(s(8.)).rounded(s(4.)).bg(color)
}

/// List row: state dot, name, then right-aligned columns of fixed width.
fn list_row(s: Scale, color: Hsla, name: String, columns: Vec<(String, f32, Hsla)>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(s(10.))
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
    let theme = StatusTheme(view.style.palette);
    let reading = match view.reading() {
        Some(FeedData::Services(services)) => Some(services),
        _ => None,
    };
    let summary = reading.map(|services| {
        let up = services
            .iter()
            .filter(|service| service.state == ServiceState::Up)
            .count();
        format!("{up}/{} up", services.len())
    });
    let Some(scroll) = view.detail_scroll else {
        return feed_panel(s, view, summary, div().into_any_element(), true)
            .justify_center()
            .gap_0();
    };
    let Some(services) = reading else {
        return feed_panel(s, view, summary, empty_body(s, view), false);
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
    let rows = services.into_iter().map(|service| {
        let color = if view.dim() {
            theme.faint()
        } else {
            service_state_color(theme, service.state)
        };
        list_row(s, color, service.name.clone(), Vec::new()).child(service_history_strip(
            s,
            theme,
            service,
            view.dim(),
        ))
    });
    let body = div()
        .id("expanded-service-list")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(list_body(s, rows));
    feed_panel(s, view, summary, body.into_any_element(), false)
}

fn disks_panel(s: Scale, view: &FeedView) -> Div {
    let theme = StatusTheme(view.style.palette);
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
            DiskState::Passed => (theme.ok(), ""),
            DiskState::FailedSmart => (theme.bad(), "SMART failed"),
            DiskState::FailedScrutiny => (theme.warn(), "threshold"),
            DiskState::FailedBoth => (theme.bad(), "failed"),
        };
        let color = if dim { theme.faint() } else { color };
        list_row(
            s,
            color,
            disk.device.clone(),
            vec![
                (
                    disk.temperature_celsius
                        .map_or(String::new(), |t| format!("{t:.0}°C")),
                    40.,
                    theme.muted(),
                ),
                (format_status_bytes(disk.capacity_bytes), 64., theme.muted()),
                (state.to_string(), 80., color),
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

fn tailnet_panel(s: Scale, view: &FeedView, carded: &CardedTailnetDevices) -> Div {
    let theme = StatusTheme(view.style.palette);
    let Some(FeedData::Tailnet(devices)) = view.reading() else {
        return feed_panel(s, view, None, empty_body(s, view), false);
    };
    let devices: Vec<_> = carded.visible_devices(devices).collect();
    let dim = view.dim();
    let online = devices.iter().filter(|d| d.online).count();
    let rows = devices.iter().map(|device| {
        let color = match (dim, device.online) {
            (true, _) | (_, false) => theme.faint(),
            (_, true) => theme.ok(),
        };
        let connection = tailnet_connection_text(view, device);
        list_row(
            s,
            color,
            device.name.clone(),
            vec![
                (device.os.clone(), 64., theme.muted()),
                (connection, 96., theme.muted()),
            ],
        )
        .when(!device.online, |row| row.text_color(theme.muted()))
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
    let theme = StatusTheme(view.style.palette);
    let Some(FeedData::HostPresence(HostPresence {
        device,
        metrics_error,
    })) = view.reading()
    else {
        return feed_panel(s, view, None, empty_body(s, view), compact);
    };
    let dim = view.dim();
    let (dot, state) = if device.online {
        (theme.ok(), "online".to_string())
    } else {
        let seen = device
            .last_seen
            .map(|_| format!("offline, seen {}", view.ago(device.last_seen)))
            .unwrap_or_else(|| "offline".into());
        (theme.faint(), seen)
    };
    let dot = if dim { theme.faint() } else { dot };
    let state_color = if device.online && !dim {
        theme.text()
    } else {
        theme.muted()
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
    let path = (device.online && !device.is_self).then(|| tailnet_connection_text(view, device));
    let Some(error) = metrics_error.as_ref().filter(|_| !compact) else {
        // Keep the state and OS on one line in the collapsed host card.
        let row = state_row.child(div().flex_none().text_color(theme.muted()).child(
            if device.online && metrics_error.is_some() {
                "No metrics".into()
            } else {
                device.os.clone()
            },
        ));
        return feed_panel(s, view, path, row.into_any_element(), true);
    };
    let body = div()
        .flex()
        .flex_col()
        .gap(s(6.))
        .text_size(s(14.))
        .child(state_row)
        .child(div().text_color(theme.muted()).child(device.os.clone()))
        .child(presence_note(theme, s, device, error, dim));
    feed_panel(s, view, path, body.into_any_element(), compact)
}

/// Note under the presence card of a Glances host: why the card shows no
/// metrics. A host that is offline is a normal state, so its note is gray.
/// An online host whose Glances does not answer gets an amber note with the
/// error.
fn presence_note(
    theme: StatusTheme,
    s: Scale,
    device: &TailnetDevice,
    metrics_error: &str,
    dim: bool,
) -> Div {
    let note = div().flex().flex_col().gap(s(2.)).text_size(s(13.));
    match metrics_error {
        _ if !device.online => note
            .text_color(theme.faint())
            .child("Metrics show again when the host is online."),
        error => note
            .child(
                div()
                    .text_color(if dim { theme.muted() } else { theme.warn() })
                    .child("Glances does not answer."),
            )
            .child(
                div()
                    .text_color(theme.faint())
                    .truncate()
                    .child(error.to_string()),
            ),
    }
}

/// Tailscale path to a device, such as `direct` or `relay lax`.
fn tailnet_connection_text(view: &FeedView, device: &TailnetDevice) -> String {
    match &device.connection {
        TailnetConnection::ThisMachine => String::new(),
        TailnetConnection::Direct => "direct".to_string(),
        TailnetConnection::Relay(relay) => format!("relay {relay}"),
        TailnetConnection::Idle => "idle".to_string(),
        TailnetConnection::Offline => match device.last_seen {
            Some(_) => view.ago(device.last_seen),
            None => "offline".to_string(),
        },
    }
}
