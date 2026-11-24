//! Status theme: colors, text styles, and the layout scale of the Rune view.

use rune::{Color, TextStyle, color::srgb_decode};

/// Design reference width. The layout scales from this width.
pub const DESIGN_WIDTH: f32 = 1600.0;

/// Converts a `0xrrggbb` sRGB value to a Rune color.
pub fn srgb_color(hex: u32) -> Color {
    let channel = |shift: u32| srgb_decode(((hex >> shift) & 0xff) as f32 / 255.0);
    Color {
        r: channel(16),
        g: channel(8),
        b: channel(0),
        a: 1.0,
    }
}

/// Status theme colors. They match the v1 GPUI theme.
pub struct StatusTheme;

impl StatusTheme {
    /// Window color without a wallpaper, and while the wallpaper loads.
    pub fn background() -> Color {
        srgb_color(0x10151c)
    }
    /// Dark layer over the wallpaper, so white text stays readable.
    pub fn scrim() -> Color {
        Color::BLACK.alpha(0.28)
    }
    pub fn panel() -> Color {
        srgb_color(0x0c1016).alpha(0.78)
    }
    pub fn panel_border() -> Color {
        Color::WHITE.alpha(0.10)
    }
    pub fn track() -> Color {
        Color::WHITE.alpha(0.10)
    }
    pub fn text() -> Color {
        srgb_color(0xeef1f4)
    }
    pub fn muted() -> Color {
        srgb_color(0x9aa4af)
    }
    pub fn faint() -> Color {
        srgb_color(0x5f6873)
    }
    pub fn ok() -> Color {
        srgb_color(0x3fb950)
    }
    pub fn warn() -> Color {
        srgb_color(0xd29922)
    }
    pub fn bad() -> Color {
        srgb_color(0xf85149)
    }
    pub fn info() -> Color {
        srgb_color(0x58a6ff)
    }
    pub fn receive() -> Color {
        srgb_color(0x58a6ff)
    }
    pub fn send() -> Color {
        srgb_color(0xbc8cff)
    }
}

/// Meter color for a used fraction: green, amber from 75 %, red from 90 %.
pub fn threshold_color(fraction: f32) -> Color {
    if fraction >= 0.9 {
        StatusTheme::bad()
    } else if fraction >= 0.75 {
        StatusTheme::warn()
    } else {
        StatusTheme::ok()
    }
}

/// Layout scale: design pixels to window points, from the window width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusScale(pub f32);

impl StatusScale {
    /// Scale for a window `width` points wide.
    pub fn from_window_width(width: f32) -> Self {
        Self((width / DESIGN_WIDTH).clamp(0.6, 2.0))
    }

    /// Converts design pixels to window points.
    pub fn px(self, value: f32) -> f32 {
        value * self.0
    }

    /// Monospace panel text at a design size and weight.
    pub fn mono(self, size: f32, weight: f32) -> TextStyle {
        TextStyle::new(self.px(size), weight).mono()
    }

    /// Sans-serif clock text at a design size and weight.
    pub fn sans(self, size: f32, weight: f32) -> TextStyle {
        TextStyle::new(self.px(size), weight)
    }
}
