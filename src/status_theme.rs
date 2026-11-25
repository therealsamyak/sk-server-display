//! Status theme: colors and the layout scale of the status display.

use gpui::{Hsla, Pixels, Window, px};

use crate::status_style::active_status_style;

/// Design reference width. The layout scales from this width.
const DESIGN_WIDTH: f32 = 1600.0;

/// Space between panels, in design pixels.
pub const PANEL_GAP: f32 = 16.0;

/// Host panel height, in design pixels, without the core grid: frame, text
/// rows, and the smallest CPU and network graphs.
pub const HOST_PANEL_BASE_HEIGHT: f32 = 330.0;

/// Height of a two-row host card, including padding and border.
pub const HOST_CARD_HEIGHT: f32 = 76.0;

/// Height of one core grid row, in design pixels.
pub const CORE_GRID_ROW_HEIGHT: f32 = 26.0;

/// Part of the window height that the panels use. The wallpaper and the
/// clock fill the rest.
const PANEL_AREA_HEIGHT: f32 = 0.64;

/// Status display colors of the active status style.
pub struct StatusTheme;

impl StatusTheme {
    /// Window color without a wallpaper, and while the wallpaper loads.
    pub fn background() -> Hsla {
        active_status_style().palette.background.status_color()
    }
    /// Layer over the whole wallpaper.
    pub fn scrim() -> Hsla {
        active_status_style().palette.scrim.status_color()
    }
    /// Layer behind each panel area of the side arrangement.
    pub fn shade() -> Hsla {
        active_status_style().palette.shade.status_color()
    }
    pub fn panel() -> Hsla {
        active_status_style().palette.panel.status_color()
    }
    pub fn panel_border() -> Hsla {
        active_status_style().palette.panel_border.status_color()
    }
    pub fn track() -> Hsla {
        active_status_style().palette.track.status_color()
    }
    pub fn text() -> Hsla {
        active_status_style().palette.text.status_color()
    }
    pub fn muted() -> Hsla {
        active_status_style().palette.muted.status_color()
    }
    pub fn faint() -> Hsla {
        active_status_style().palette.faint.status_color()
    }
    pub fn ok() -> Hsla {
        active_status_style().palette.ok.status_color()
    }
    pub fn warn() -> Hsla {
        active_status_style().palette.warn.status_color()
    }
    pub fn bad() -> Hsla {
        active_status_style().palette.bad.status_color()
    }
    pub fn info() -> Hsla {
        active_status_style().palette.info.status_color()
    }
    pub fn receive() -> Hsla {
        active_status_style().palette.receive.status_color()
    }
    pub fn send() -> Hsla {
        active_status_style().palette.send.status_color()
    }
    /// Clock text color.
    pub fn clock() -> Hsla {
        active_status_style().palette.clock.status_color()
    }
    /// Date text color.
    pub fn date() -> Hsla {
        active_status_style().palette.date.status_color()
    }
}

/// Scale function: design pixels to window pixels.
pub type Scale<'a> = &'a dyn Fn(f32) -> Pixels;

/// Layout scale of one window: the UI scale and the panel area height.
#[derive(Clone, Copy, Debug)]
pub struct StatusLayout {
    /// Window pixels for one design pixel.
    pub ui: f32,
    /// Height of the panel area, in design pixels.
    pub panel_area: f32,
    /// Window width, in design pixels.
    pub width: f32,
}

impl StatusLayout {
    /// Reads the layout scale from the window size.
    pub fn from_status_window(window: &Window) -> Self {
        let viewport = window.viewport_size();
        let ui = (f32::from(viewport.width) / DESIGN_WIDTH).clamp(0.6, 2.0);
        Self {
            ui,
            panel_area: f32::from(viewport.height) * PANEL_AREA_HEIGHT / ui,
            width: f32::from(viewport.width) / ui,
        }
    }

    /// Converts design pixels to window pixels.
    pub fn scale(self, value: f32) -> Pixels {
        px(value * self.ui)
    }

    /// Height of the expanded host card, capped on tall displays.
    pub fn host_detail_height(self, window: &Window) -> Pixels {
        (window.viewport_size().height * 0.86).min(self.scale(560.))
    }
}

/// Meter and graph color for a used fraction: green, then amber from 75 %,
/// then red from 90 %.
pub fn threshold_color(fraction: f32) -> Hsla {
    if fraction >= 0.9 {
        StatusTheme::bad()
    } else if fraction >= 0.75 {
        StatusTheme::warn()
    } else {
        StatusTheme::ok()
    }
}
