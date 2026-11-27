//! Btop window: a movable and resizable btop-style view of one host, drawn
//! over the wallpaper. It reads the same feed as the host card, so it needs
//! no terminal and no connection to the host beyond its metrics source.

use gpui::{
    Context, DragMoveEvent, EmptyView, Entity, EventEmitter, FontWeight, Hsla, IntoElement,
    MouseButton, MouseDownEvent, Pixels, Point, Render, Role, Size, Subscription, Window, div,
    point, prelude::*, px, relative, size,
};

use crate::{
    dashboard::status_format::{format_status_bytes, format_status_rate, format_status_uptime},
    dashboard::status_graph::{cpu_graph, cpu_history_scale, network_graph, network_history_scale},
    dashboard::status_theme::{Scale, StatusLayout, StatusTheme, threshold_color},
    metrics::{FeedData, HostMetrics, StatusBoard, StatusFeedUpdated},
    wallpaper::StatusStyle,
};

/// Smallest btop window size, in window pixels.
const BTOP_MIN_SIZE: Size<Pixels> = size(px(520.), px(360.));

/// Part of the window that a new btop window covers.
const BTOP_START_FRACTION: f32 = 0.64;

/// Btop window view of the host feed at `index`.
pub struct BtopWindowView {
    board: Entity<StatusBoard>,
    index: usize,
    style: StatusStyle,
    origin: Point<Pixels>,
    size: Size<Pixels>,
    /// Pointer offset from the dragged corner when a move or resize starts.
    grab: Point<Pixels>,
    _subscription: Subscription,
}

/// The user closed the btop window.
pub struct BtopWindowClosed;

impl EventEmitter<BtopWindowClosed> for BtopWindowView {}

/// Drag of the btop title bar: moves the window.
struct BtopMoveDrag;

/// Drag of the btop corner grip: resizes the window.
struct BtopResizeDrag;

impl BtopWindowView {
    /// Opens a centered btop window for the host feed at `index`.
    pub fn new_btop_window(
        board: Entity<StatusBoard>,
        index: usize,
        style: StatusStyle,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&board, |this, _, event: &StatusFeedUpdated, cx| {
            if event.index == this.index {
                cx.notify();
            }
        });
        let viewport = window.viewport_size();
        let size = size(
            (viewport.width * BTOP_START_FRACTION).max(BTOP_MIN_SIZE.width),
            (viewport.height * BTOP_START_FRACTION).max(BTOP_MIN_SIZE.height),
        )
        .min(&viewport);
        Self {
            board,
            index,
            style,
            origin: point(
                (viewport.width - size.width) / 2.,
                (viewport.height - size.height) / 2.,
            ),
            size,
            grab: Point::default(),
            _subscription: subscription,
        }
    }

    /// Feed index of the host in this window.
    pub fn btop_feed_index(&self) -> usize {
        self.index
    }

    /// Set the style of the wallpaper on screen.
    pub fn set_btop_style(&mut self, style: StatusStyle, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    fn move_to(&mut self, pointer: Point<Pixels>, viewport: Size<Pixels>) {
        let origin = pointer - self.grab;
        self.origin = point(
            origin
                .x
                .clamp(px(0.), (viewport.width - self.size.width).max(px(0.))),
            origin
                .y
                .clamp(px(0.), (viewport.height - self.size.height).max(px(0.))),
        );
    }

    fn resize_to(&mut self, pointer: Point<Pixels>, viewport: Size<Pixels>) {
        let corner = pointer - self.grab;
        self.size = size(
            (corner.x - self.origin.x).clamp(BTOP_MIN_SIZE.width, viewport.width - self.origin.x),
            (corner.y - self.origin.y).clamp(BTOP_MIN_SIZE.height, viewport.height - self.origin.y),
        );
    }
}

impl Render for BtopWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = StatusLayout::from_status_window(window);
        let scale = move |value: f32| layout.scale(value);
        let s: Scale = &scale;
        let theme = StatusTheme(self.style.palette);
        let surface = Hsla {
            a: 0.96,
            ..theme.background()
        };
        let board = self.board.read(cx);
        let feed = &board.status_feeds()[self.index];
        let title = feed.spec.title.clone();
        let host = match feed.latest_reading() {
            Some(FeedData::Host(host)) => Some(host),
            _ => None,
        };
        let history = feed.host_history();
        let summary = host
            .map(|host| {
                [
                    host.uptime_seconds
                        .map(|uptime| format!("up {}", format_status_uptime(uptime))),
                    host.load_1m.map(|load| format!("load {load:.2}")),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ")
            })
            .unwrap_or_default();
        let body = match host {
            Some(host) => {
                let cpu_scale = cpu_history_scale(&history.cpu_percent);
                let network_ceiling = network_history_scale(history);
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap(s(16.))
                    .child(
                        div()
                            .flex()
                            .gap(s(12.))
                            .min_h_0()
                            .flex_grow(0.85)
                            .flex_basis(relative(0.))
                            .child(
                                btop_box(s, surface, "cpu", theme.ok(), 3.)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_x(s(8.))
                                            .justify_between()
                                            .text_size(s(13.))
                                            .child(format!(
                                                "Total {}",
                                                percent_text(host.cpu_percent)
                                            ))
                                            .child(
                                                div()
                                                    .text_color(theme.faint())
                                                    .child(cpu_scale.scale_label()),
                                            )
                                            .children(host.temperature_celsius.map(|celsius| {
                                                div()
                                                    .text_color(theme.muted())
                                                    .child(format!("{celsius:.0}°C"))
                                            })),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_h(s(32.))
                                            .max_h(s(140.))
                                            .flex()
                                            .flex_col()
                                            .child(cpu_graph(
                                                s,
                                                theme,
                                                &history.cpu_percent,
                                                cpu_scale,
                                                false,
                                            )),
                                    )
                                    .child(core_meters(s, theme, &host.core_percents)),
                            )
                            .child(
                                btop_box(s, surface, "mem", theme.info(), 2.)
                                    .child(memory_rows(s, theme, host)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(s(12.))
                            .min_h_0()
                            .flex_grow(1.25)
                            .flex_basis(relative(0.))
                            .child(
                                btop_box(s, surface, "net", theme.receive(), 2.)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_x(s(8.))
                                            .justify_between()
                                            .text_size(s(13.))
                                            .child(div().text_color(theme.receive()).child(
                                                format!(
                                                    "↓ {}",
                                                    rate_text(host.network_receive_rate)
                                                ),
                                            ))
                                            .child(div().text_color(theme.send()).child(format!(
                                                "↑ {}",
                                                rate_text(host.network_send_rate)
                                            )))
                                            .child(div().text_color(theme.faint()).child(format!(
                                                "Scale {}",
                                                format_status_rate(network_ceiling)
                                            ))),
                                    )
                                    .child(network_graph(
                                        s,
                                        theme,
                                        history,
                                        network_ceiling,
                                        false,
                                    )),
                            )
                            .child(
                                btop_box(s, surface, "proc", theme.warn(), 3.)
                                    .child(process_table(s, theme, host)),
                            ),
                    )
                    .into_any_element()
            }
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted())
                .child("Waiting for host metrics")
                .into_any_element(),
        };
        let viewport = window.viewport_size();
        div()
            .id("btop-window")
            .absolute()
            .left(self.origin.x)
            .top(self.origin.y)
            .w(self.size.width)
            .h(self.size.height)
            .occlude()
            .role(Role::Dialog)
            .aria_label(format!("btop for {title}"))
            .flex()
            .flex_col()
            .rounded(s(10.))
            .border_1()
            .border_color(theme.panel_border())
            .bg(surface)
            .text_color(theme.text())
            .overflow_hidden()
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<BtopMoveDrag>, _, cx| {
                    this.move_to(event.event.position, viewport);
                    cx.notify();
                }),
            )
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<BtopResizeDrag>, _, cx| {
                    this.resize_to(event.event.position, viewport);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("btop-title-bar")
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(s(12.))
                    .px(s(12.))
                    .py(s(8.))
                    .border_b_1()
                    .border_color(theme.panel_border())
                    .cursor_grab()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, _| {
                            this.grab = event.position - this.origin;
                        }),
                    )
                    .on_drag(BtopMoveDrag, |_, _, _, cx| cx.new(|_| EmptyView))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(s(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("btop · {title}")),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(s(13.))
                            .text_color(theme.muted())
                            .child(summary),
                    )
                    .child(
                        div()
                            .id("close-btop")
                            .role(Role::Button)
                            .flex_none()
                            .h(s(24.))
                            .px(s(8.))
                            .flex()
                            .items_center()
                            .rounded(s(6.))
                            .border_1()
                            .border_color(theme.panel_border())
                            .text_size(s(13.))
                            .cursor_pointer()
                            .hover(move |button| button.bg(theme.track()))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(BtopWindowClosed)))
                            .child("Close"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .pt(s(18.))
                    .px(s(12.))
                    .pb(s(12.))
                    .child(body),
            )
            .child(
                div()
                    .id("btop-resize-grip")
                    .absolute()
                    .right_0()
                    .bottom_0()
                    .size(s(24.))
                    .cursor(gpui::CursorStyle::ResizeUpLeftDownRight)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, _| {
                            let corner = this.origin + point(this.size.width, this.size.height);
                            this.grab = event.position - corner;
                        }),
                    )
                    .on_drag(BtopResizeDrag, |_, _, _, cx| cx.new(|_| EmptyView))
                    .child(
                        div()
                            .absolute()
                            .right(s(6.))
                            .bottom(s(6.))
                            .size(s(10.))
                            .border_r_2()
                            .border_b_2()
                            .border_color(theme.muted()),
                    ),
            )
    }
}

/// Box with its title in the top border, as btop draws its boxes.
fn btop_box(s: Scale, surface: Hsla, title: &'static str, accent: Hsla, grow: f32) -> gpui::Div {
    let mut frame = div()
        .relative()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .gap(s(8.))
        .pt(s(14.))
        .px(s(10.))
        .pb(s(10.))
        .rounded(s(6.))
        .border_1()
        .border_color(accent.opacity(0.7))
        .child(
            div()
                .absolute()
                .top(-s(10.))
                .left(s(10.))
                .px(s(4.))
                .bg(surface)
                .text_size(s(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(accent)
                .child(title),
        );
    frame.style().flex_grow = Some(grow);
    frame.style().flex_basis = Some(relative(0.).into());
    frame
}

fn percent_text(percent: Option<f32>) -> String {
    percent.map_or("—".into(), |percent| format!("{percent:.0}%"))
}

fn rate_text(rate: Option<f64>) -> String {
    rate.map_or("—".into(), format_status_rate)
}

/// Horizontal usage bar with a threshold color.
fn usage_bar(s: Scale, theme: StatusTheme, fraction: f32) -> gpui::Div {
    let fraction = fraction.clamp(0., 1.);
    div()
        .flex_1()
        .min_w(s(24.))
        .h(s(8.))
        .rounded(s(4.))
        .bg(theme.track())
        .child(
            div()
                .h_full()
                .rounded(s(4.))
                .w(relative(fraction))
                .bg(threshold_color(theme, fraction)),
        )
}

/// One bar for each logical core, in rows that wrap with the box width.
fn core_meters(s: Scale, theme: StatusTheme, cores: &[f32]) -> gpui::Div {
    div()
        .flex()
        .flex_wrap()
        .gap_x(s(16.))
        .gap_y(s(4.))
        .overflow_hidden()
        .text_size(s(12.))
        .children(cores.iter().enumerate().map(|(index, percent)| {
            div()
                .w(s(150.))
                .flex()
                .items_center()
                .gap(s(6.))
                .child(
                    div()
                        .w(s(28.))
                        .text_color(theme.muted())
                        .child(format!("C{index}")),
                )
                .child(usage_bar(s, theme, percent / 100.))
                .child(
                    div()
                        .w(s(36.))
                        .flex()
                        .justify_end()
                        .child(format!("{percent:.0}%")),
                )
        }))
}

/// Memory, disk, and temperature rows of the mem box.
fn memory_rows(s: Scale, theme: StatusTheme, host: &HostMetrics) -> gpui::Div {
    let usage_row = |label: &'static str, used: Option<u64>, total: Option<u64>| {
        let (fraction, text) = match (used, total) {
            (Some(used), Some(total)) if total > 0 => (
                used as f32 / total as f32,
                format!(
                    "{} / {}",
                    format_status_bytes(used),
                    format_status_bytes(total)
                ),
            ),
            _ => (0., "unavailable".into()),
        };
        div()
            .flex()
            .flex_col()
            .gap(s(4.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(label)
                    .child(div().text_color(theme.muted()).child(text)),
            )
            .child(div().flex().child(usage_bar(s, theme, fraction)))
    };
    div()
        .flex()
        .flex_col()
        .gap(s(12.))
        .text_size(s(13.))
        .child(usage_row(
            "Used",
            host.memory_used_bytes,
            host.memory_total_bytes,
        ))
        .child(usage_row(
            "Disk",
            host.disk_used_bytes,
            host.disk_total_bytes,
        ))
}

/// Busiest processes, highest CPU use first.
fn process_table(s: Scale, theme: StatusTheme, host: &HostMetrics) -> gpui::Div {
    let row = || div().flex().items_center().gap(s(10.)).text_size(s(13.));
    let header = row()
        .text_color(theme.muted())
        .child(div().w(s(64.)).child("Pid"))
        .child(div().flex_1().min_w_0().child("Program"))
        .child(div().w(s(64.)).flex().justify_end().child("Cpu%"))
        .child(div().w(s(80.)).flex().justify_end().child("Mem"));
    let table = div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap(s(2.))
        .overflow_hidden()
        .child(header);
    if host.processes.is_empty() {
        return table.child(
            div()
                .text_size(s(13.))
                .text_color(theme.muted())
                .child("This source does not report processes."),
        );
    }
    table.children(host.processes.iter().map(|process| {
        row()
            .child(
                div()
                    .w(s(64.))
                    .text_color(theme.muted())
                    .child(process.pid.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(process.name.clone()),
            )
            .child(
                div()
                    .w(s(64.))
                    .flex()
                    .justify_end()
                    .text_color(threshold_color(theme, process.cpu_percent / 100.))
                    .child(format!("{:.1}", process.cpu_percent)),
            )
            .child(
                div()
                    .w(s(80.))
                    .flex()
                    .justify_end()
                    .child(format_status_bytes(process.memory_bytes)),
            )
    }))
}
