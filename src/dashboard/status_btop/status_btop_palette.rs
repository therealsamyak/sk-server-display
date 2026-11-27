//! Btop window colors derived from the active wallpaper palette.

use gpui::{Hsla, Rgba, rgb};

use crate::{dashboard::status_theme::StatusTheme, wallpaper::StatusStyle};

const TEXT_CONTRAST: f32 = 4.5;

#[derive(Clone, Copy)]
pub(super) struct BtopPalette {
    /// Text, accent, and meter colors inside the btop window.
    pub(super) theme: StatusTheme,
    /// Fill for the btop window.
    pub(super) surface: Hsla,
    /// Opacity of the colored btop section borders.
    pub(super) section_border_opacity: f32,
}

impl BtopPalette {
    /// Adapt the active wallpaper colors to a separate btop window.
    pub(super) fn from_wallpaper(style: StatusStyle) -> Self {
        let mut palette = style.palette;
        let panel = rgb(palette.panel.0);
        let panel_luminance = relative_luminance(panel);
        if panel_luminance <= relative_luminance(rgb(palette.text.0)) {
            return Self {
                theme: StatusTheme(palette),
                surface: palette.background.status_color().alpha(0.96),
                section_border_opacity: 0.7,
            };
        }

        // Light presets choose their text for the pale panel. The window color
        // belongs behind the wallpaper and can be a much more saturated hue.
        for tone in [
            &mut palette.text,
            &mut palette.muted,
            &mut palette.faint,
            &mut palette.ok,
            &mut palette.warn,
            &mut palette.bad,
            &mut palette.info,
            &mut palette.receive,
            &mut palette.send,
        ] {
            tone.0 = readable_ink(tone.0, panel_luminance);
            tone.1 = 1.0;
        }
        palette.panel_border.1 = palette.panel_border.1.max(0.36);

        Self {
            theme: StatusTheme(palette),
            surface: panel.into(),
            section_border_opacity: 0.85,
        }
    }
}

/// Darken the ink toward black while keeping its hue and saturation.
fn readable_ink(color: u32, panel_luminance: f32) -> u32 {
    let ink = rgb(color);
    let maximum_luminance = (panel_luminance + 0.05) / TEXT_CONTRAST - 0.05;
    if relative_luminance(ink) <= maximum_luminance {
        return color;
    }

    let black = rgb(0);
    let (mut too_light, mut readable) = (0.0, 1.0);
    for _ in 0..8 {
        let amount = f32::midpoint(too_light, readable);
        if relative_luminance(ink.blend(black.alpha(amount))) <= maximum_luminance {
            readable = amount;
        } else {
            too_light = amount;
        }
    }

    u32::from(ink.blend(black.alpha(readable))) >> 8
}

/// WCAG relative luminance for a color in sRGB.
fn relative_luminance(color: Rgba) -> f32 {
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallpaper::{StatusCarousel, StatusPlayback, WallpaperId, plain_status_style};

    #[test]
    fn btop_light_presets_keep_text_and_plot_marks_readable() {
        let carousel = StatusCarousel::new_status_carousel(StatusPlayback::default(), None, None);
        for name in ["focalors-blossom", "yolanda-nahida"] {
            let id: WallpaperId = serde_json::from_value(name.into()).unwrap();
            let style = carousel.wallpaper_style(id);
            let palette = BtopPalette::from_wallpaper(style);
            let surface = Rgba::from(palette.surface);
            assert_eq!(
                palette.surface,
                style.palette.panel.status_color().alpha(1.0)
            );

            let theme = palette.theme;
            let text = [theme.text(), theme.muted(), theme.faint()];
            let accents = [
                theme.ok(),
                theme.warn(),
                theme.bad(),
                theme.info(),
                theme.receive(),
                theme.send(),
            ];
            let contrast = |ink: Hsla| {
                (relative_luminance(surface) + 0.05)
                    / (relative_luminance(surface.blend(ink.into())) + 0.05)
            };
            for color in text.into_iter().chain(accents) {
                let ratio = contrast(color);
                assert!(ratio >= 4.5, "{name}: text contrast {ratio:.2}");
            }
            for color in accents {
                let ratio = contrast(color.opacity(palette.section_border_opacity));
                assert!(ratio >= 3.0, "{name}: section border contrast {ratio:.2}");
                let ratio = contrast(color.opacity(0.85));
                assert!(ratio >= 3.0, "{name}: plot mark contrast {ratio:.2}");
            }
        }
    }

    #[test]
    fn btop_dark_palette_preserves_the_existing_glass_and_ink() {
        let style = plain_status_style();
        let palette = BtopPalette::from_wallpaper(style);
        let original = StatusTheme(style.palette);
        let rgba = |color: Hsla| u32::from(Rgba::from(color));
        assert_eq!(
            rgba(palette.surface),
            rgba(original.background().alpha(0.96))
        );
        assert_eq!(rgba(palette.theme.text()), rgba(original.text()));
        assert_eq!(rgba(palette.theme.faint()), rgba(original.faint()));
        assert_eq!(rgba(palette.theme.ok()), rgba(original.ok()));
        assert_eq!(
            rgba(palette.theme.panel_border()),
            rgba(original.panel_border())
        );
        assert_eq!(palette.section_border_opacity, 0.7);
    }
}
