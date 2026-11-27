//! Status theme: colors and the layout scale of the status display.

use gpui::{Hsla, Pixels, Window, px};

use crate::wallpaper::StylePalette;

/// Design reference width. The layout scales from this width.
const DESIGN_WIDTH: f32 = 1600.0;

/// Space between panels, in design pixels.
pub const PANEL_GAP: f32 = 16.0;

/// Height of a two-row host card, including padding and border.
pub const HOST_CARD_HEIGHT: f32 = 76.0;

/// Part of the window height that the panels use. The wallpaper and the
/// clock fill the rest.
const PANEL_AREA_HEIGHT: f32 = 0.64;

/// Status display colors of a rendered status style.
#[derive(Clone, Copy)]
pub struct StatusTheme(pub StylePalette);

impl StatusTheme {
    /// Window color without a wallpaper, and while the wallpaper loads.
    pub fn background(self) -> Hsla {
        self.0.background.status_color()
    }
    /// Layer over the whole wallpaper.
    pub fn scrim(self) -> Hsla {
        self.0.scrim.status_color()
    }
    /// Layer behind each panel area of the side arrangement.
    pub fn shade(self) -> Hsla {
        self.0.shade.status_color()
    }
    pub fn panel(self) -> Hsla {
        self.0.panel.status_color()
    }
    pub fn panel_border(self) -> Hsla {
        self.0.panel_border.status_color()
    }
    pub fn track(self) -> Hsla {
        self.0.track.status_color()
    }
    pub fn text(self) -> Hsla {
        self.0.text.status_color()
    }
    pub fn muted(self) -> Hsla {
        self.0.muted.status_color()
    }
    pub fn faint(self) -> Hsla {
        self.0.faint.status_color()
    }
    pub fn ok(self) -> Hsla {
        self.0.ok.status_color()
    }
    pub fn warn(self) -> Hsla {
        self.0.warn.status_color()
    }
    pub fn bad(self) -> Hsla {
        self.0.bad.status_color()
    }
    pub fn info(self) -> Hsla {
        self.0.info.status_color()
    }
    pub fn receive(self) -> Hsla {
        self.0.receive.status_color()
    }
    pub fn send(self) -> Hsla {
        self.0.send.status_color()
    }
    /// Clock text color.
    pub fn clock(self) -> Hsla {
        self.0.clock.status_color()
    }
    /// Date text color.
    pub fn date(self) -> Hsla {
        self.0.date.status_color()
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
}

/// Meter and graph color for a used fraction: green, then amber from 75 %,
/// then red from 90 %.
pub fn threshold_color(theme: StatusTheme, fraction: f32) -> Hsla {
    if fraction >= 0.9 {
        theme.bad()
    } else if fraction >= 0.75 {
        theme.warn()
    } else {
        theme.ok()
    }
}
