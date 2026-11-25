//! Status styles for wallpapers that won official HoYoverse art contests:
//! the Honkai: Star Rail "Multiverse Vistas" creator competition from 2024 to
//! 2026, and the "Genshin Impact" and Honkai: Star Rail illustration contests on
//! pixiv. Each style follows one wallpaper. The palette uses colors from the
//! art, and the panels sit where the art is quiet.
//!
//! The colors and areas are fixed values, picked by hand for each wallpaper.
//! No image analysis runs at run time.

use crate::status_fonts::{
    STATUS_MONO_FONT_FAMILY, STATUS_SANS_FONT_FAMILY, STATUS_SERIF_FONT_FAMILY,
};
use crate::status_style::{
    PanelGroup, StatusStyle, StyleAnchor, StyleArea, StyleArrangement, StylePalette, StyleTone,
    StyleType,
};

/// Opaque tone from a hex color.
const fn hex(value: u32) -> StyleTone {
    StyleTone(value, 1.0)
}

const NO_AREA: StyleArea = StyleArea {
    width: 0.0,
    clock: None,
    groups: &[],
    panels: StyleAnchor::Top,
};

/// The Trailblazers walk across a star chart: navy glass at both edges, chart
/// white type, and the pink of the rabbit ears for upload.
pub const TRAILBLAZERS_ORBIT: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x232a3d),
        scrim: StyleTone(0x232a3d, 0.08),
        shade: StyleTone(0x232a3d, 0.55),
        panel: StyleTone(0x1d2233, 0.72),
        panel_border: StyleTone(0xa9b3c9, 0.24),
        track: StyleTone(0xe8ecf5, 0.14),
        text: hex(0xe8ecf5),
        muted: hex(0xa9b3c9),
        faint: hex(0x6c7590),
        ok: hex(0x9fdcc0),
        warn: hex(0xe8c87a),
        bad: hex(0xff7d7d),
        info: hex(0x9ec3f0),
        receive: hex(0x9ec3f0),
        send: hex(0xe0a3c8),
        clock: hex(0xf4f6fb),
        date: hex(0xa9b3c9),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Cyrene draws a bow of light before a red star: plum glass at both edges
/// and the magenta of the bow.
pub const CYRENE_BOW: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x2a0f24),
        scrim: StyleTone(0x2a0f24, 0.08),
        shade: StyleTone(0x2a0f24, 0.55),
        panel: StyleTone(0x260c22, 0.72),
        panel_border: StyleTone(0xd4a8c4, 0.24),
        track: StyleTone(0xf8e6f1, 0.14),
        text: hex(0xf8e6f1),
        muted: hex(0xd4a8c4),
        faint: hex(0x8f6283),
        ok: hex(0xa8e0c4),
        warn: hex(0xf2c98a),
        bad: hex(0xff7a90),
        info: hex(0xf1cfe6),
        receive: hex(0xe8b6d6),
        send: hex(0xf08ac4),
        clock: hex(0xfff4fa),
        date: hex(0xd4a8c4),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Pearl paints a stream of water for March 7th and Stelle: teal glass at the
/// left, over the dark stage, and water white type.
pub const PEARL_WATER: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x1f3442),
        scrim: StyleTone(0x1f3442, 0.08),
        shade: StyleTone(0x1f3442, 0.55),
        panel: StyleTone(0x1a2d3a, 0.72),
        panel_border: StyleTone(0xa1bec3, 0.24),
        track: StyleTone(0xe6f2f4, 0.14),
        text: hex(0xe6f2f4),
        muted: hex(0xa1bec3),
        faint: hex(0x6a8891),
        ok: hex(0x9fe0c8),
        warn: hex(0xf0d28a),
        bad: hex(0xff8a80),
        info: hex(0x8fd3e6),
        receive: hex(0x8fd3e6),
        send: hex(0xe9a0b0),
        clock: hex(0xf5fbfc),
        date: hex(0xa1bec3),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.44,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts, PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
        right: NO_AREA,
    },
};

/// Stelle among glass shards with the Stellaron Hunters: navy glass at both
/// edges and the gold of the light she holds.
pub const STELLE_SHARDS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x25273f),
        scrim: StyleTone(0x25273f, 0.08),
        shade: StyleTone(0x25273f, 0.55),
        panel: StyleTone(0x1f2238, 0.74),
        panel_border: StyleTone(0xb4b2c6, 0.24),
        track: StyleTone(0xeceaf4, 0.14),
        text: hex(0xeceaf4),
        muted: hex(0xb4b2c6),
        faint: hex(0x75738f),
        ok: hex(0xa0dcc0),
        warn: hex(0xf0cc7a),
        bad: hex(0xff7a7a),
        info: hex(0xa9b8f0),
        receive: hex(0xa9b8f0),
        send: hex(0xe9a46a),
        clock: hex(0xfff8e6),
        date: hex(0xc9b98a),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Sparxie and Sparkle on a rose stage: rose glass at both edges, round
/// corners, and the pink of the stage lights.
pub const SPARXIE_STAGE: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x3d1c25),
        scrim: StyleTone(0x3d1c25, 0.08),
        shade: StyleTone(0x3d1c25, 0.55),
        panel: StyleTone(0x4a1f2c, 0.72),
        panel_border: StyleTone(0xeab9c4, 0.24),
        track: StyleTone(0xfff0f3, 0.14),
        text: hex(0xfff0f3),
        muted: hex(0xeab9c4),
        faint: hex(0xa8737f),
        ok: hex(0xa8e6c8),
        warn: hex(0xffd28a),
        bad: hex(0xff5c5c),
        info: hex(0xffd0dc),
        receive: hex(0xffc2d1),
        send: hex(0xff7a92),
        clock: hex(0xffffff),
        date: hex(0xeab9c4),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 8.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Kafka and the Stellaron Hunters over a city of fireworks: dark violet
/// glass at the right, over the far towers, and neon pink.
pub const KAFKA_CITY: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x261829),
        scrim: StyleTone(0x261829, 0.08),
        shade: StyleTone(0x261829, 0.55),
        panel: StyleTone(0x1f1322, 0.74),
        panel_border: StyleTone(0xc8a9c4, 0.24),
        track: StyleTone(0xf5e8f2, 0.14),
        text: hex(0xf5e8f2),
        muted: hex(0xc8a9c4),
        faint: hex(0x86688a),
        ok: hex(0x9fe0c4),
        warn: hex(0xf2cf8a),
        bad: hex(0xff6f86),
        info: hex(0xe8a6dc),
        receive: hex(0xd9a8f0),
        send: hex(0xff7ab8),
        clock: hex(0xffffff),
        date: hex(0xc8a9c4),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: NO_AREA,
        right: StyleArea {
            width: 0.44,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Others, PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Aglaea, Cipher, and Tribbie rest in a sunny meadow: pale sky cards at both
/// edges, navy type, and Tribbie red for upload.
pub const AGLAEA_MEADOW: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x9cc5f6),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xeef5fc, 0.32),
        panel: StyleTone(0xf3f7fc, 0.84),
        panel_border: StyleTone(0x273347, 0.16),
        track: StyleTone(0x273347, 0.12),
        text: hex(0x273347),
        muted: hex(0x55657d),
        faint: hex(0x93a0b3),
        ok: hex(0x2f7d5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x3a6ea8),
        receive: hex(0x3a6ea8),
        send: hex(0xb0606a),
        clock: hex(0x273347),
        date: hex(0x55657d),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Castorice among violet ribbons and butterflies: violet glass at both edges
/// and lilac type.
pub const CASTORICE_VIOLET: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x2a1f55),
        scrim: StyleTone(0x2a1f55, 0.08),
        shade: StyleTone(0x2a1f55, 0.55),
        panel: StyleTone(0x241a4a, 0.72),
        panel_border: StyleTone(0xcbbcec, 0.24),
        track: StyleTone(0xf3eeff, 0.14),
        text: hex(0xf3eeff),
        muted: hex(0xcbbcec),
        faint: hex(0x8a7cb8),
        ok: hex(0xa0e2c6),
        warn: hex(0xf2d48e),
        bad: hex(0xff7a96),
        info: hex(0xd8c8ff),
        receive: hex(0xc6b0f5),
        send: hex(0xf0a0e0),
        clock: hex(0xffffff),
        date: hex(0xcbbcec),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Cyrene rests her head among flowers in glass jars: blush cards at both
/// edges, plum type, and the violet of her book.
pub const CYRENE_FLOWERS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xf2d7d7),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xfff6f4, 0.32),
        panel: StyleTone(0xfff7f5, 0.84),
        panel_border: StyleTone(0x5a3346, 0.16),
        track: StyleTone(0x5a3346, 0.12),
        text: hex(0x4a2a3a),
        muted: hex(0x7d5869),
        faint: hex(0xb496a3),
        ok: hex(0x2f7d5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x6a5a9c),
        receive: hex(0x6a5a9c),
        send: hex(0xc0607a),
        clock: hex(0x5a3346),
        date: hex(0x7d5869),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Robin sings above a night city: indigo glass at the right, over the cage,
/// and the gold of the notes.
pub const ROBIN_CONCERT: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x1e1b3d),
        scrim: StyleTone(0x1e1b3d, 0.08),
        shade: StyleTone(0x1e1b3d, 0.55),
        panel: StyleTone(0x1a1838, 0.72),
        panel_border: StyleTone(0xb8b0d8, 0.24),
        track: StyleTone(0xeeeaff, 0.14),
        text: hex(0xeeeaff),
        muted: hex(0xb8b0d8),
        faint: hex(0x7a7399),
        ok: hex(0x9fdcc4),
        warn: hex(0xf0cf86),
        bad: hex(0xff7c8c),
        info: hex(0xa6b8ff),
        receive: hex(0xa6b8ff),
        send: hex(0xd6a6f0),
        clock: hex(0xffffff),
        date: hex(0xb8b0d8),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: NO_AREA,
        right: StyleArea {
            width: 0.44,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Others, PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Sparkle with a fox mask among red maple leaves: warm dark glass at the
/// left, over the shrine, and maple orange.
pub const SPARKLE_MAPLE: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x3a2530),
        scrim: StyleTone(0x3a2530, 0.08),
        shade: StyleTone(0x3a2530, 0.55),
        panel: StyleTone(0x2e1d27, 0.74),
        panel_border: StyleTone(0xe0b9a6, 0.24),
        track: StyleTone(0xfbeee4, 0.14),
        text: hex(0xfbeee4),
        muted: hex(0xe0b9a6),
        faint: hex(0x9c7a73),
        ok: hex(0xa6dcb8),
        warn: hex(0xf6c06a),
        bad: hex(0xff6a5a),
        info: hex(0xf6be98),
        receive: hex(0xf6be98),
        send: hex(0xe38a69),
        clock: hex(0xfff4ea),
        date: hex(0xe0b9a6),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.44,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts, PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
        right: NO_AREA,
    },
};

/// Acheron with a red blade before a black eclipse: black glass at both
/// edges, white type, one red, square corners, and Plex Mono.
pub const ACHERON_ECLIPSE: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x161112),
        scrim: StyleTone(0x161112, 0.08),
        shade: StyleTone(0x161112, 0.55),
        panel: StyleTone(0x120d0e, 0.76),
        panel_border: StyleTone(0xe8e9f5, 0.20),
        track: StyleTone(0xeeeff6, 0.14),
        text: hex(0xeeeff6),
        muted: hex(0xa6a9b8),
        faint: hex(0x676a78),
        ok: hex(0xb8d8c8),
        warn: hex(0xe0c890),
        bad: hex(0xff4a55),
        info: hex(0xc8cce0),
        receive: hex(0xc8cce0),
        send: hex(0xd22a3a),
        clock: hex(0xffffff),
        date: hex(0xa6a9b8),
    },
    kind: StyleType {
        clock_family: STATUS_MONO_FONT_FAMILY,
        clock_light: false,
        clock_size: 112.0,
        title_family: STATUS_MONO_FONT_FAMILY,
        title_semibold: true,
        radius: 0.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Silver Wolf and Kafka in a pale morning: lilac cards at the right, over
/// the bright sky, and navy type.
pub const HUNTERS_DAYLIGHT: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xe6d8e6),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xfbf6f8, 0.32),
        panel: StyleTone(0xfcf8fa, 0.84),
        panel_border: StyleTone(0x2e3466, 0.16),
        track: StyleTone(0x2e3466, 0.12),
        text: hex(0x2a2f5e),
        muted: hex(0x5c5f8c),
        faint: hex(0x9a9bbb),
        ok: hex(0x2f7d5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x4a5fb0),
        receive: hex(0x4a5fb0),
        send: hex(0xb0407a),
        clock: hex(0x2a2f5e),
        date: hex(0x5c5f8c),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: NO_AREA,
        right: StyleArea {
            width: 0.44,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Others, PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Lumine and Paimon rest on white linen: paper cards at the right, over the
/// empty sheet, and warm ink type.
pub const LUMINE_LINEN: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xf4efea),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xfffdfa, 0.32),
        panel: StyleTone(0xfffcf8, 0.84),
        panel_border: StyleTone(0x3a3548, 0.16),
        track: StyleTone(0x3a3548, 0.12),
        text: hex(0x2e2b3c),
        muted: hex(0x645f72),
        faint: hex(0xa7a2b0),
        ok: hex(0x2f7d5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x4a5a8a),
        receive: hex(0x4a5a8a),
        send: hex(0xb0705a),
        clock: hex(0x3a3548),
        date: hex(0x8a7060),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: StyleArrangement::Sides {
        left: NO_AREA,
        right: StyleArea {
            width: 0.44,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Others, PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
    },
};
