//! Status graphs: meters, the CPU history graph, the network history graph,
//! and the per-core grid. Graphs draw as one Rune shape each.

use std::collections::VecDeque;

use rune::{Color, El, Shape, col, el, row, text};
use server_status_core::{
    status_feed::{HOST_HISTORY_SAMPLES, HostHistory},
    status_format::format_status_rate,
};

use crate::status_theme::{StatusScale, StatusTheme, threshold_color};

/// Lowest top of the network graph scale, so idle noise stays flat.
pub const NETWORK_GRAPH_FLOOR: f64 = 64.0 * 1024.0;

/// Part of a graph slot that its bar fills. The rest is the gap.
const GRAPH_BAR_WIDTH: f32 = 0.7;

/// Horizontal meter: a track and a bar for a used fraction.
pub fn status_meter(s: StatusScale, fraction: Option<f32>, height: f32, dim: bool) -> El {
    let fraction = fraction.map(|f| f.clamp(0.0, 1.0));
    let color = match (fraction, dim) {
        (_, true) | (None, _) => StatusTheme::faint(),
        (Some(fraction), false) => threshold_color(fraction),
    };
    let radius = s.px(height / 2.0);
    el().grow(1.0)
        .h(s.px(height))
        .radius(radius)
        .fill(StatusTheme::track())
        .child(
            el().h_full()
                .w_frac(fraction.unwrap_or(0.0))
                .radius(radius)
                .fill(color),
        )
}

/// One bar of a graph in a view box `HOST_HISTORY_SAMPLES` wide: slot
/// `slot`, from `y_from` to `y_to`.
fn graph_bar(shape: Shape, slot: usize, y_from: f32, y_to: f32, color: Color) -> Shape {
    let left = slot as f32 + (1.0 - GRAPH_BAR_WIDTH) / 2.0;
    let right = left + GRAPH_BAR_WIDTH;
    let (top, bottom) = (y_from.min(y_to), y_from.max(y_to));
    shape.fill(
        vec![[left, top], [right, top], [right, bottom], [left, bottom]],
        color,
    )
}

/// First graph slot of the oldest sample, so the newest sample is on the right.
fn first_graph_slot(samples: usize) -> usize {
    HOST_HISTORY_SAMPLES.saturating_sub(samples)
}

/// CPU history graph: one bar for each sample, in the meter colors.
pub fn cpu_history_graph(s: StatusScale, samples: &VecDeque<f32>, dim: bool) -> El {
    let first = first_graph_slot(samples.len());
    let mut shape = Shape::new().view_box(HOST_HISTORY_SAMPLES as f32, 100.0);
    for (offset, percent) in samples.iter().enumerate() {
        let percent = percent.clamp(0.0, 100.0);
        let color = if dim {
            StatusTheme::faint()
        } else {
            threshold_color(percent / 100.0)
        };
        let height = percent.max(1.0);
        shape = graph_bar(
            shape,
            first + offset,
            100.0 - height,
            100.0,
            color.alpha(0.85),
        );
    }
    el().grow(1.0)
        .min_h(0.0)
        .w_full()
        .shape(shape)
        .border_sides([0.0, 0.0, s.px(1.0), 0.0], StatusTheme::track())
}

/// Network history graph: receive bars rise from the middle line and send
/// bars fall from it. `peak` is the scale, which shows in the corner.
pub fn network_history_graph(s: StatusScale, history: &HostHistory, peak: f64, dim: bool) -> El {
    let mut shape = Shape::new().view_box(HOST_HISTORY_SAMPLES as f32, 200.0);
    let color = |color: Color| if dim { StatusTheme::faint() } else { color };
    let height = |rate: f64| ((rate / peak).clamp(0.0, 1.0) * 100.0).max(0.5) as f32;
    let receive = &history.network_receive_rate;
    let first = first_graph_slot(receive.len());
    for (offset, rate) in receive.iter().enumerate() {
        let bar = color(StatusTheme::receive()).alpha(0.85);
        shape = graph_bar(shape, first + offset, 100.0 - height(*rate), 100.0, bar);
    }
    let send = &history.network_send_rate;
    let first = first_graph_slot(send.len());
    for (offset, rate) in send.iter().enumerate() {
        let bar = color(StatusTheme::send()).alpha(0.85);
        shape = graph_bar(shape, first + offset, 100.0, 100.0 + height(*rate), bar);
    }
    shape = shape.fill(
        vec![[0.0, 99.75], [90.0, 99.75], [90.0, 100.25], [0.0, 100.25]],
        StatusTheme::track(),
    );
    el().grow(1.0).min_h(0.0).w_full().shape(shape).child(
        text(format_status_rate(peak))
            .font(s.mono(12.0, 400.0))
            .color(StatusTheme::faint())
            .absolute(0.0, 0.0),
    )
}

/// Number of columns of the core grid.
pub fn core_grid_columns(cores: &[f32]) -> usize {
    if cores.len() > 8 { 4 } else { 2 }
}

/// Per-core usage in a grid of small meters.
pub fn core_usage_grid(s: StatusScale, cores: &[f32], dim: bool) -> El {
    let columns = core_grid_columns(cores);
    let rows = cores.chunks(columns).enumerate().map(|(row_index, chunk)| {
        row()
            .w_full()
            .gap(s.px(24.0))
            .children(chunk.iter().enumerate().map(|(column, percent)| {
                let index = row_index * columns + column;
                row()
                    .grow(1.0)
                    .items_center()
                    .gap(s.px(8.0))
                    .child(
                        text(index.to_string())
                            .font(s.mono(13.0, 400.0))
                            .color(StatusTheme::faint())
                            .w(s.px(24.0)),
                    )
                    .child(status_meter(s, Some(percent / 100.0), 4.0, dim))
                    .child(
                        row().w(s.px(40.0)).justify_end().child(
                            text(format!("{percent:.0}%"))
                                .font(s.mono(13.0, 400.0))
                                .color(StatusTheme::muted()),
                        ),
                    )
            }))
    });
    col().w_full().gap(s.px(6.0)).children(rows)
}
