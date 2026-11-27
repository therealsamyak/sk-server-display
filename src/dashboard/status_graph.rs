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
    dashboard::status_theme::{Scale, StatusTheme, threshold_color},
    metrics::{HOST_HISTORY_SAMPLES, HostHistory},
};

const CPU_SCALE_STEP: f32 = 10.0;
const NETWORK_SCALE_FLOOR: f64 = 4.0 * 1024.0;

/// CPU history ceiling and whether any samples exceed it.
#[derive(Clone, Copy)]
pub struct CpuHistoryScale {
    ceiling: f32,
    has_peaks_above_ceiling: bool,
}

impl CpuHistoryScale {
    /// Label the CPU scale and flag bars that exceed the ceiling.
    pub fn scale_label(self) -> String {
        let marker = if self.has_peaks_above_ceiling {
            "+"
        } else {
            ""
        };
        format!("Scale {:.0}%{marker}", self.ceiling)
    }
}

/// Round the 90th percentile of CPU history up to the next ten-point step.
pub fn cpu_history_scale(samples: &VecDeque<f32>) -> CpuHistoryScale {
    let mut values = [0.0; HOST_HISTORY_SAMPLES];
    let count = samples
        .iter()
        .copied()
        .filter(|percent| percent.is_finite())
        .zip(values.iter_mut())
        .map(|(percent, slot)| *slot = percent.clamp(0.0, 100.0))
        .count();
    if count == 0 {
        return CpuHistoryScale {
            ceiling: CPU_SCALE_STEP,
            has_peaks_above_ceiling: false,
        };
    }
    let rank = count - count / 10 - 1;
    let (_, percentile, peaks) = values[..count].select_nth_unstable_by(rank, f32::total_cmp);
    let ceiling = (percentile.max(CPU_SCALE_STEP) / CPU_SCALE_STEP).ceil() * CPU_SCALE_STEP;
    CpuHistoryScale {
        ceiling,
        has_peaks_above_ceiling: peaks.iter().any(|peak| *peak > ceiling),
    }
}

/// Use one power-of-two rate scale for both directions of network history.
pub fn network_history_scale(history: &HostHistory) -> f64 {
    let peak = history
        .network_receive_rate
        .iter()
        .chain(&history.network_send_rate)
        .copied()
        .filter(|rate| rate.is_finite())
        .fold(NETWORK_SCALE_FLOOR, f64::max);
    let ceiling = peak.log2().ceil().exp2();
    if ceiling.is_finite() {
        ceiling.max(peak)
    } else {
        peak
    }
}

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
    peak_marker: Option<Hsla>,
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
        if let Some(marker) = peak_marker
            && *fraction > 1.0
        {
            let marker_height = (device_pixel * 2.0).min(height);
            window.paint_quad(fill(
                Bounds::new(point(left, y), size(right - left, marker_height)),
                marker,
            ));
        }
    }
}

/// Canvas that paints one set of history bars and fills its parent.
fn history_canvas(
    gap: Pixels,
    bars: Vec<GraphBar>,
    rising: bool,
    peak_marker: Option<Hsla>,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            paint_history_bars(bounds, gap, &bars, rising, peak_marker, window);
        },
    )
    .flex_1()
    .min_h_0()
}

/// CPU history graph: one bar for each sample, newest on the right.
pub fn cpu_graph(
    s: Scale,
    theme: StatusTheme,
    samples: &VecDeque<f32>,
    scale: CpuHistoryScale,
    dim: bool,
) -> impl IntoElement {
    let bars = samples
        .iter()
        .map(|percent| {
            let percent = if percent.is_finite() {
                percent.clamp(0.0, 100.0)
            } else {
                0.0
            };
            let color = if dim {
                theme.faint()
            } else {
                threshold_color(theme, percent / 100.0)
            };
            (percent / scale.ceiling, color.opacity(GRAPH_BAR_OPACITY))
        })
        .collect();
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .border_b_1()
        .border_color(theme.track())
        .child(history_canvas(
            s(2.),
            bars,
            true,
            Some(if dim { theme.faint() } else { theme.text() }),
        ))
}

/// Network history graph: receive bars rise from the middle line and send
/// bars fall from it. Both directions use the same rate ceiling.
pub fn network_graph(
    s: Scale,
    theme: StatusTheme,
    history: &HostHistory,
    ceiling: f64,
    dim: bool,
) -> impl IntoElement {
    let bars = |samples: &VecDeque<f64>, color: Hsla| {
        let color = if dim { theme.faint() } else { color };
        samples
            .iter()
            .map(|rate| {
                let rate = if rate.is_finite() { *rate } else { 0.0 };
                (
                    (rate / ceiling).clamp(0.0, 1.0) as f32,
                    color.opacity(GRAPH_BAR_OPACITY),
                )
            })
            .collect()
    };
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(history_canvas(
            s(2.),
            bars(&history.network_receive_rate, theme.receive()),
            true,
            None,
        ))
        .child(div().h(px(1.)).bg(theme.track()))
        .child(history_canvas(
            s(2.),
            bars(&history.network_send_rate, theme.send()),
            false,
            None,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_scale_keeps_idle_history_on_a_valid_axis() {
        let samples = VecDeque::from([0.0, -1.0, f32::NAN, f32::INFINITY]);
        assert!((cpu_history_scale(&samples).ceiling - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cpu_scale_rounds_up_to_fit_a_short_history() {
        let samples = VecDeque::from([2.0, 23.0, 5.0]);
        assert!((cpu_history_scale(&samples).ceiling - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cpu_scale_marks_isolated_spikes_without_flattening_typical_load() {
        let samples = VecDeque::from([2.0, 4.0, 6.0, 2.0, 3.0, 100.0, 5.0, 7.0, 4.0, 3.0]);
        let scale = cpu_history_scale(&samples);
        assert!((scale.ceiling - 10.0).abs() < f32::EPSILON);
        assert!(scale.has_peaks_above_ceiling);
    }

    #[test]
    fn cpu_scale_grows_for_sustained_load() {
        let samples = VecDeque::from([95.0; HOST_HISTORY_SAMPLES]);
        let scale = cpu_history_scale(&samples);
        assert!((scale.ceiling - 100.0).abs() < f32::EPSILON);
        assert!(!scale.has_peaks_above_ceiling);
    }

    #[test]
    fn cpu_scale_stays_within_one_hundred_percent() {
        let samples = VecDeque::from([105.0]);
        assert!((cpu_history_scale(&samples).ceiling - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn network_scale_keeps_idle_history_on_a_valid_axis() {
        let history = HostHistory {
            network_receive_rate: VecDeque::from([f64::NAN, 97.0]),
            network_send_rate: VecDeque::from([f64::INFINITY, 1000.0]),
            ..HostHistory::default()
        };
        assert!((network_history_scale(&history) - 4096.0).abs() < f64::EPSILON);
    }

    #[test]
    fn network_scale_includes_the_peak_from_either_direction() {
        let mut history = HostHistory {
            network_receive_rate: VecDeque::from([4_000.0]),
            network_send_rate: VecDeque::from([17_000.0]),
            ..HostHistory::default()
        };
        assert!((network_history_scale(&history) - 32768.0).abs() < f64::EPSILON);
        std::mem::swap(
            &mut history.network_receive_rate,
            &mut history.network_send_rate,
        );
        assert!((network_history_scale(&history) - 32768.0).abs() < f64::EPSILON);
    }
}
