//! Status theme: colors and the layout scale of the status display.

use gpui::{Hsla, Pixels, Window, px, rgb};

/// Design reference width. The layout scales from this width.
const DESIGN_WIDTH: f32 = 1600.0;

/// Space between panels, in design pixels.
pub const PANEL_GAP: f32 = 16.0;

/// Host panel height, in design pixels, without the core grid: frame, text
/// rows, and the smallest CPU and network graphs.
pub const HOST_PANEL_BASE_HEIGHT: f32 = 330.0;

/// Height of a status card: a host card with the online state only, in
/// design pixels. Padding, the title row, and one state row.
pub const PRESENCE_PANEL_HEIGHT: f32 = 76.0;

/// Height of one core grid row, in design pixels.
pub const CORE_GRID_ROW_HEIGHT: f32 = 26.0;

/// Part of the window height that the panels use. The wallpaper and the
/// clock fill the rest.
const PANEL_AREA_HEIGHT: f32 = 0.64;

/// Status display colors.
pub struct StatusTheme;

impl StatusTheme {
    /// Window color without a wallpaper, and while the wallpaper loads.
    pub fn background() -> Hsla {
        rgb(0x10151c).into()
    }
    /// Dark layer over the wallpaper, so white text stays readable.
    pub fn scrim() -> Hsla {
        Hsla::black().opacity(0.28)
    }
    pub fn panel() -> Hsla {
        Hsla::from(rgb(0x0c1016)).opacity(0.78)
    }
    pub fn panel_border() -> Hsla {
        Hsla::white().opacity(0.10)
    }
    pub fn track() -> Hsla {
        Hsla::white().opacity(0.10)
    }
    pub fn text() -> Hsla {
        rgb(0xeef1f4).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x9aa4af).into()
    }
    pub fn faint() -> Hsla {
        rgb(0x5f6873).into()
    }
    pub fn ok() -> Hsla {
        rgb(0x3fb950).into()
    }
    pub fn warn() -> Hsla {
        rgb(0xd29922).into()
    }
    pub fn bad() -> Hsla {
        rgb(0xf85149).into()
    }
    pub fn info() -> Hsla {
        rgb(0x58a6ff).into()
    }
    pub fn receive() -> Hsla {
        rgb(0x58a6ff).into()
    }
    pub fn send() -> Hsla {
        rgb(0xbc8cff).into()
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
}

impl StatusLayout {
    /// Reads the layout scale from the window size.
    pub fn from_status_window(window: &Window) -> Self {
        let viewport = window.viewport_size();
        let ui = (f32::from(viewport.width) / DESIGN_WIDTH).clamp(0.6, 2.0);
        Self {
            ui,
            panel_area: f32::from(viewport.height) * PANEL_AREA_HEIGHT / ui,
        }
    }

    /// Converts design pixels to window pixels.
    pub fn scale(self, value: f32) -> Pixels {
        px(value * self.ui)
    }

    /// Host panel height, in design pixels, when `host_rows` panels share
    /// the host column.
    pub fn host_panel_height(self, host_rows: usize) -> f32 {
        (self.panel_area - PANEL_GAP * (host_rows as f32 - 1.0)) / host_rows.max(1) as f32
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
