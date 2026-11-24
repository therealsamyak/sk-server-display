//! Status graphs: CPU and network history, painted as quads on a canvas.
//!
//! One canvas paints all bars of a graph, as gpui-component charts do. A
//! graph is one layout node, not one node for each bar.

use std::collections::VecDeque;

use gpui::{
    Bounds, Hsla, IntoElement, ParentElement as _, Pixels, Styled as _, Window, canvas, div, fill,
    point, px, size,
};

use crate::{
    status_feed::{HOST_HISTORY_SAMPLES, HostHistory},
    status_format::format_status_rate,
    status_theme::{Scale, StatusTheme, threshold_color},
};

/// Lowest top of the network graph scale, so idle noise stays flat.
pub const NETWORK_GRAPH_FLOOR: f64 = 64.0 * 1024.0;

/// Opacity of the graph bars over the panel color.
const GRAPH_BAR_OPACITY: f32 = 0.85;

/// One graph bar: its height as a part of the graph height, and its color.
type GraphBar = (f32, Hsla);

/// Paints history bars, newest on the right. Bars rise from the bottom edge,
/// or fall from the top edge when `rising` is `false`.
fn paint_history_bars(
    bounds: Bounds<Pixels>,
    gap: Pixels,
    bars: &[GraphBar],
    rising: bool,
    window: &mut Window,
) {
    // Bars snap to whole device pixels and are at least one pixel wide. A
    // narrow graph has slots thinner than one pixel, and the GPU draws a
    // pixel only when a quad covers the pixel center.
    let scale = window.scale_factor();
    let snap = |value: Pixels| px((f32::from(value) * scale).round() / scale);
    let device_pixel = px(1.0 / scale);
    let slots = HOST_HISTORY_SAMPLES as f32;
    let slot_width = (bounds.size.width - gap * (slots - 1.0)) / slots;
    let first_slot = HOST_HISTORY_SAMPLES.saturating_sub(bars.len());
    let (top, bottom) = (snap(bounds.origin.y), snap(bounds.bottom()));
    for (offset, (fraction, color)) in bars.iter().enumerate() {
        let left = snap(bounds.origin.x + (slot_width + gap) * (first_slot + offset) as f32);
        let right = snap(left + slot_width).max(left + device_pixel);
        let height = snap((bottom - top) * fraction.clamp(0.0, 1.0)).max(device_pixel);
        let y = if rising { bottom - height } else { top };
        window.paint_quad(fill(
            Bounds::new(point(left, y), size(right - left, height)),
            *color,
        ));
    }
}

/// Canvas that paints one set of history bars and fills its parent.
fn history_canvas(gap: Pixels, bars: Vec<GraphBar>, rising: bool) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| paint_history_bars(bounds, gap, &bars, rising, window),
    )
    .flex_1()
    .min_h_0()
}

/// CPU history graph: one bar for each sample, newest on the right.
pub fn cpu_graph(s: Scale, samples: &VecDeque<f32>, dim: bool) -> impl IntoElement {
    let bars = samples
        .iter()
        .map(|percent| {
            let fraction = (percent / 100.0).clamp(0.0, 1.0);
            let color = if dim {
                StatusTheme::faint()
            } else {
                threshold_color(fraction)
            };
            (fraction, color.opacity(GRAPH_BAR_OPACITY))
        })
        .collect();
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .border_b_1()
        .border_color(StatusTheme::track())
        .child(history_canvas(s(2.), bars, true))
}

/// Network history graph: receive bars rise from the middle line and send
/// bars fall from it. The scale is the peak rate of the shown samples.
pub fn network_graph(s: Scale, history: &HostHistory, peak: f64, dim: bool) -> impl IntoElement {
    let bars = |samples: &VecDeque<f64>, color: Hsla| {
        let color = if dim { StatusTheme::faint() } else { color };
        samples
            .iter()
            .map(|rate| {
                (
                    (rate / peak).clamp(0.0, 1.0) as f32,
                    color.opacity(GRAPH_BAR_OPACITY),
                )
            })
            .collect()
    };
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(history_canvas(
            s(2.),
            bars(&history.network_receive_rate, StatusTheme::receive()),
            true,
        ))
        .child(div().h(px(1.)).bg(StatusTheme::track()))
        .child(history_canvas(
            s(2.),
            bars(&history.network_send_rate, StatusTheme::send()),
            false,
        ))
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
