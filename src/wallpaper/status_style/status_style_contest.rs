//! Status styles for wallpapers by artists from official HoYoverse art
//! contests and HoYoFair on pixiv. Each style follows one wallpaper. The palette uses
//! colors from the art, and the panels sit where the art is quiet.
//!
//! The colors and areas are fixed values, picked by hand for each wallpaper.
//! No image analysis runs at run time.

use crate::status_fonts::{
    STATUS_MONO_FONT_FAMILY, STATUS_SANS_FONT_FAMILY, STATUS_SERIF_FONT_FAMILY,
};
use crate::wallpaper::status_style::{
    FAVORITE_CORNER_WIDTHS, PanelGroup, SANS_TYPE, StatusStyle, StyleAnchor, StylePalette,
    StyleTone, StyleType, corner_arrangement,
};

/// Opaque tone from a hex color.
const fn hex(value: u32) -> StyleTone {
    StyleTone(value, 1.0)
}

/// Furina in a balloon over misty Fontaine: pale mist cards at the right, teal
/// type, and the red of the balloon for upload.
pub const FURINA_BALLOON: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xcfd9d7),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xeef3f1, 0.32),
        panel: StyleTone(0xeef3f1, 0.84),
        panel_border: StyleTone(0x24323a, 0.16),
        track: StyleTone(0x24323a, 0.12),
        text: hex(0x24323a),
        muted: hex(0x566a6e),
        faint: hex(0x97a6a8),
        ok: hex(0x2f7d5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x3d7f8c),
        receive: hex(0x3d7f8c),
        send: hex(0xc0524a),
        clock: hex(0x2b3a40),
        date: hex(0x566a6e),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Rosaria in flooded ruins: slate glass at both dark edges, the blue glow of
/// the glyph, and the red of her sash.
pub const ROSARIA_RUINS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x1f2528),
        scrim: StyleTone(0x1f2528, 0.08),
        shade: StyleTone(0x1f2528, 0.55),
        panel: StyleTone(0x151b1e, 0.72),
        panel_border: StyleTone(0xa2adb6, 0.24),
        track: StyleTone(0xe6edf2, 0.14),
        text: hex(0xe6edf2),
        muted: hex(0xa2adb6),
        faint: hex(0x5f6a72),
        ok: hex(0x8fd0b0),
        warn: hex(0xe8c06a),
        bad: hex(0xff6b62),
        info: hex(0x7fc4e8),
        receive: hex(0x7fc4e8),
        send: hex(0xe06a62),
        clock: hex(0xe6edf2),
        date: hex(0xa2adb6),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 4.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.24]),
};

/// Lumine in a sunlit cathedral: warm umber glass between the pillars, gold
/// type, and a serif clock.
pub const LUMINE_CATHEDRAL: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x3a302b),
        scrim: StyleTone(0x3a302b, 0.08),
        shade: StyleTone(0x3a302b, 0.55),
        panel: StyleTone(0x2a221e, 0.72),
        panel_border: StyleTone(0xdcc8b0, 0.24),
        track: StyleTone(0xfbf5ea, 0.14),
        text: hex(0xfbf5ea),
        muted: hex(0xdcc8b0),
        faint: hex(0x8f8072),
        ok: hex(0xa6d6a0),
        warn: hex(0xf2c46d),
        bad: hex(0xff7a6a),
        info: hex(0xe8c77a),
        receive: hex(0xe8c77a),
        send: hex(0xf0a070),
        clock: hex(0xfffaf0),
        date: hex(0xe8c77a),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 2.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.24]),
};

/// Ayaka in a moonlit room: ink glass in the dark at both edges, blue from the
/// butterflies, and red from her ribbons.
pub const AYAKA_MOONLIT: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x0e131e),
        scrim: StyleTone(0x0e131e, 0.08),
        shade: StyleTone(0x0e131e, 0.55),
        panel: StyleTone(0x0b111c, 0.72),
        panel_border: StyleTone(0xa8b4c8, 0.24),
        track: StyleTone(0xe9eef7, 0.14),
        text: hex(0xe9eef7),
        muted: hex(0xa8b4c8),
        faint: hex(0x5d6a80),
        ok: hex(0x8fd6b8),
        warn: hex(0xf0c46a),
        bad: hex(0xff6b78),
        info: hex(0x7fb0e8),
        receive: hex(0x7fb0e8),
        send: hex(0xd65a6a),
        clock: hex(0xe9eef7),
        date: hex(0xa8b4c8),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 4.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.25]),
};

/// Keqing in a wedding dress: lilac cards over the bright sky at the left, violet
/// type.
pub const KEQING_WEDDING: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xbbcae5),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xf7f5fb, 0.32),
        panel: StyleTone(0xf7f5fb, 0.84),
        panel_border: StyleTone(0x2a2347, 0.16),
        track: StyleTone(0x2a2347, 0.12),
        text: hex(0x2a2347),
        muted: hex(0x5f5689),
        faint: hex(0xa6a0c4),
        ok: hex(0x2f8a5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0394b),
        info: hex(0x6d5cc0),
        receive: hex(0x6d5cc0),
        send: hex(0x3f8fc0),
        clock: hex(0x3b2e6e),
        date: hex(0x5f5689),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 14.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Bronya on a snowy field: ice cards over the blurred army at the right, navy
/// type, and the gold of her coat.
pub const BRONYA_SNOW: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xa2b9d3),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xf3f6fb, 0.32),
        panel: StyleTone(0xf3f6fb, 0.84),
        panel_border: StyleTone(0x1e2a4a, 0.16),
        track: StyleTone(0x1e2a4a, 0.12),
        text: hex(0x1e2a4a),
        muted: hex(0x4e5d82),
        faint: hex(0x98a4c0),
        ok: hex(0x2f8a5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x2f5fb0),
        receive: hex(0x2f5fb0),
        send: hex(0xb8862c),
        clock: hex(0x1e2a4a),
        date: hex(0x4e5d82),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 6.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Fu Xuan among constellations: violet glass over the starfield at both
/// edges, star lines in pale lilac.
pub const FU_XUAN_STARS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x121732),
        scrim: StyleTone(0x121732, 0.08),
        shade: StyleTone(0x121732, 0.55),
        panel: StyleTone(0x151a3a, 0.72),
        panel_border: StyleTone(0xbcb6dd, 0.24),
        track: StyleTone(0xefeaff, 0.14),
        text: hex(0xefeaff),
        muted: hex(0xbcb6dd),
        faint: hex(0x6f6a9a),
        ok: hex(0x8be0c0),
        warn: hex(0xf5c26b),
        bad: hex(0xff6b8b),
        info: hex(0xa68cff),
        receive: hex(0xa68cff),
        send: hex(0xf2a9d8),
        clock: hex(0xefeaff),
        date: hex(0xbcb6dd),
    },
    kind: SANS_TYPE,
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, FAVORITE_CORNER_WIDTHS),
};

/// Nahida in the Sumeru forest: moss glass over the trees at the left, dendro
/// green and sun gold.
pub const NAHIDA_FOREST: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x2c3018),
        scrim: StyleTone(0x2c3018, 0.08),
        shade: StyleTone(0x2c3018, 0.55),
        panel: StyleTone(0x1f2614, 0.72),
        panel_border: StyleTone(0xcfd4a8, 0.24),
        track: StyleTone(0xf3f2e2, 0.14),
        text: hex(0xf3f2e2),
        muted: hex(0xcfd4a8),
        faint: hex(0x7f8a5a),
        ok: hex(0xb6e36a),
        warn: hex(0xf2c97a),
        bad: hex(0xff7a6a),
        info: hex(0xb6e36a),
        receive: hex(0xb6e36a),
        send: hex(0xf2c97a),
        clock: hex(0xf8f6e4),
        date: hex(0xcfd4a8),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 16.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Focalors in blue blossoms: pale water cards and deep blue
/// type.
pub const FOCALORS_BLOSSOM: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xb6d0f1),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xf4f9ff, 0.32),
        panel: StyleTone(0xf4f9ff, 0.84),
        panel_border: StyleTone(0x14284a, 0.16),
        track: StyleTone(0x14284a, 0.12),
        text: hex(0x14284a),
        muted: hex(0x4a628c),
        faint: hex(0x98aac8),
        ok: hex(0x2f8a5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x2f6fd0),
        receive: hex(0x2f6fd0),
        send: hex(0x3aa0b8),
        clock: hex(0x1c3a78),
        date: hex(0x4a628c),
    },
    kind: SANS_TYPE,
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, FAVORITE_CORNER_WIDTHS),
};

/// Kirara on a delivery ride: cream cards over the town at the left, dusk navy
/// type, and her cat green.
pub const KIRARA_DELIVERY: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xd4cac9),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xfbf7f2, 0.32),
        panel: StyleTone(0xfbf7f2, 0.84),
        panel_border: StyleTone(0x2d3550, 0.16),
        track: StyleTone(0x2d3550, 0.12),
        text: hex(0x2d3550),
        muted: hex(0x5e678b),
        faint: hex(0xa8aec4),
        ok: hex(0x3f9a52),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x3f9a52),
        receive: hex(0x3f9a52),
        send: hex(0xd86a8a),
        clock: hex(0x3b4570),
        date: hex(0x5e678b),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 16.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Furina in an open sky: navy glass over the sky at the left, white type, and
/// the blue of her roses.
pub const FURINA_SKY: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x314da1),
        scrim: StyleTone(0x314da1, 0.08),
        shade: StyleTone(0x314da1, 0.55),
        panel: StyleTone(0x14245a, 0.72),
        panel_border: StyleTone(0xc8daf2, 0.24),
        track: StyleTone(0xf5f9ff, 0.14),
        text: hex(0xf5f9ff),
        muted: hex(0xc8daf2),
        faint: hex(0x8aa6d0),
        ok: hex(0x8fe0c0),
        warn: hex(0xffd98a),
        bad: hex(0xff7a8a),
        info: hex(0x9fd2ff),
        receive: hex(0x9fd2ff),
        send: hex(0xffd98a),
        clock: hex(0xffffff),
        date: hex(0xdbe8fa),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 12.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Bottom, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Ruan Mei by a plum window: blush cards in the light at the left, plum
/// type, and the teal of her dress.
pub const RUAN_MEI_BLOSSOM: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xe6cfd8),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xfff6f8, 0.32),
        panel: StyleTone(0xfff6f8, 0.84),
        panel_border: StyleTone(0x3a2a3c, 0.16),
        track: StyleTone(0x3a2a3c, 0.12),
        text: hex(0x3a2a3c),
        muted: hex(0x6e5a70),
        faint: hex(0xb09aa8),
        ok: hex(0x2f8a5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0394b),
        info: hex(0x2f8a9a),
        receive: hex(0x2f8a9a),
        send: hex(0xd0708e),
        clock: hex(0x423548),
        date: hex(0x6e5a70),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 10.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.24]),
};

/// Mavuika on a desert ride: sand cards under the sky at the right, ember
/// orange, and sky teal.
pub const MAVUIKA_RIDE: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xcfc4ae),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xfbf6ee, 0.32),
        panel: StyleTone(0xfbf6ee, 0.84),
        panel_border: StyleTone(0x3a2a22, 0.16),
        track: StyleTone(0x3a2a22, 0.12),
        text: hex(0x3a2a22),
        muted: hex(0x7a6250),
        faint: hex(0xb4a28e),
        ok: hex(0x2f8a5a),
        warn: hex(0xb06d00),
        bad: hex(0xc62828),
        info: hex(0x2f8f94),
        receive: hex(0x2f8f94),
        send: hex(0xd2502a),
        clock: hex(0x49352d),
        date: hex(0x7a6250),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 8.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Castorice in a violet garden: dusk glass over the trunks at the left, lilac
/// serif type.
pub const CASTORICE_GARDEN: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x42428b),
        scrim: StyleTone(0x42428b, 0.08),
        shade: StyleTone(0x42428b, 0.55),
        panel: StyleTone(0x23224f, 0.72),
        panel_border: StyleTone(0xc9c2ea, 0.24),
        track: StyleTone(0xf3eff7, 0.14),
        text: hex(0xf3eff7),
        muted: hex(0xc9c2ea),
        faint: hex(0x8a84bc),
        ok: hex(0x8fe0c0),
        warn: hex(0xf5c26b),
        bad: hex(0xff6b8b),
        info: hex(0xb9a6ff),
        receive: hex(0xb9a6ff),
        send: hex(0xf2a8c8),
        clock: hex(0xf3eff7),
        date: hex(0xc9c2ea),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 14.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Yae Miko in a misty forest: paper cards in the mist at the left, ink type, and
/// one shrine red.
pub const YAE_MIST: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0xbdbdba),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xf6f5f3, 0.32),
        panel: StyleTone(0xf6f5f3, 0.84),
        panel_border: StyleTone(0x2f2626, 0.16),
        track: StyleTone(0x2f2626, 0.12),
        text: hex(0x2f2626),
        muted: hex(0x6b5a58),
        faint: hex(0xa9a09e),
        ok: hex(0x3a8a5a),
        warn: hex(0xa86a10),
        bad: hex(0xb23a48),
        info: hex(0xb23a48),
        receive: hex(0xb23a48),
        send: hex(0xd97a8a),
        clock: hex(0x5a2f34),
        date: hex(0xb23a48),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 2.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Shenhe and Ganyu under lanterns: amber glass at both warm edges, lantern
/// gold, and the ice blue of Shenhe.
pub const SHENHE_LANTERNS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x3a2a22),
        scrim: StyleTone(0x3a2a22, 0.08),
        shade: StyleTone(0x3a2a22, 0.55),
        panel: StyleTone(0x2a1d16, 0.72),
        panel_border: StyleTone(0xe0c2a2, 0.24),
        track: StyleTone(0xfff3e4, 0.14),
        text: hex(0xfff3e4),
        muted: hex(0xe0c2a2),
        faint: hex(0x9a7c62),
        ok: hex(0xa6d6a0),
        warn: hex(0xffcf7a),
        bad: hex(0xff7a6a),
        info: hex(0xffcf7a),
        receive: hex(0xffcf7a),
        send: hex(0x8fc8e8),
        clock: hex(0xfff3e4),
        date: hex(0xe0c2a2),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 8.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.23, 0.23]),
};

/// Furina sinking in deep water: teal glass at both edges, caustic aqua.
pub const FURINA_DEEP: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x1a2c4f),
        scrim: StyleTone(0x1a2c4f, 0.08),
        shade: StyleTone(0x1a2c4f, 0.55),
        panel: StyleTone(0x0f1f3a, 0.72),
        panel_border: StyleTone(0xa8d4da, 0.24),
        track: StyleTone(0xe8f8fa, 0.14),
        text: hex(0xe8f8fa),
        muted: hex(0xa8d4da),
        faint: hex(0x5f8a9a),
        ok: hex(0x7fe0c0),
        warn: hex(0xf2c46d),
        bad: hex(0xff7a8a),
        info: hex(0x3ac8d0),
        receive: hex(0x3ac8d0),
        send: hex(0x9ad0ff),
        clock: hex(0xe8f8fa),
        date: hex(0xa8d4da),
    },
    kind: SANS_TYPE,
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, FAVORITE_CORNER_WIDTHS),
};

/// Kafka in a ruined city: graphite glass over the debris at both edges, steel
/// type, and her magenta.
pub const KAFKA_RUINS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x1b1a1f),
        scrim: StyleTone(0x1b1a1f, 0.08),
        shade: StyleTone(0x1b1a1f, 0.55),
        panel: StyleTone(0x18171c, 0.72),
        panel_border: StyleTone(0xb4b8c8, 0.24),
        track: StyleTone(0xeef0f6, 0.14),
        text: hex(0xeef0f6),
        muted: hex(0xb4b8c8),
        faint: hex(0x6c6f80),
        ok: hex(0x9fd0b0),
        warn: hex(0xe8c06a),
        bad: hex(0xff5a6a),
        info: hex(0xe06a9a),
        receive: hex(0xe06a9a),
        send: hex(0x9da6be),
        clock: hex(0xeef0f6),
        date: hex(0xe06a9a),
    },
    kind: StyleType {
        clock_family: STATUS_MONO_FONT_FAMILY,
        clock_light: false,
        clock_size: 112.0,
        title_family: STATUS_MONO_FONT_FAMILY,
        title_semibold: true,
        radius: 0.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.25]),
};

/// March 7th and Stelle at a snowy shrine: cocoa glass at both edges, lantern
/// amber, and March pink.
pub const MARCH_SHRINE: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x2c282f),
        scrim: StyleTone(0x2c282f, 0.08),
        shade: StyleTone(0x2c282f, 0.55),
        panel: StyleTone(0x221d22, 0.72),
        panel_border: StyleTone(0xd4b8aa, 0.24),
        track: StyleTone(0xf8efe8, 0.14),
        text: hex(0xf8efe8),
        muted: hex(0xd4b8aa),
        faint: hex(0x8a7068),
        ok: hex(0xa6d6a0),
        warn: hex(0xf0b27a),
        bad: hex(0xff6b6b),
        info: hex(0xf0b27a),
        receive: hex(0xf0b27a),
        send: hex(0xe88aa0),
        clock: hex(0xfff4ea),
        date: hex(0xd4b8aa),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 10.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.24]),
};

/// Acheron in red rain: ash glass at the left, gray type, and one blood red.
pub const ACHERON_RAIN: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x332c2e),
        scrim: StyleTone(0x332c2e, 0.08),
        shade: StyleTone(0x332c2e, 0.55),
        panel: StyleTone(0x1e1a1b, 0.72),
        panel_border: StyleTone(0xc8bcbc, 0.24),
        track: StyleTone(0xf4f0f0, 0.14),
        text: hex(0xf4f0f0),
        muted: hex(0xc8bcbc),
        faint: hex(0x7a6e70),
        ok: hex(0x9fd0b0),
        warn: hex(0xe8c06a),
        bad: hex(0xe04a5a),
        info: hex(0xe04a5a),
        receive: hex(0xe04a5a),
        send: hex(0xc8bcbc),
        clock: hex(0xf4f0f0),
        date: hex(0xe04a5a),
    },
    kind: StyleType {
        clock_family: STATUS_MONO_FONT_FAMILY,
        clock_light: false,
        clock_size: 112.0,
        title_family: STATUS_MONO_FONT_FAMILY,
        title_semibold: true,
        radius: 0.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Odette on a starry floor (2026): navy glass over the dark sky at the left,
/// pale blue type, and the glow of her dress.
pub const ODETTE_STARS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x0a0c22),
        scrim: StyleTone(0x0a0c22, 0.08),
        shade: StyleTone(0x0a0c22, 0.55),
        panel: StyleTone(0x0c1030, 0.74),
        panel_border: StyleTone(0xa9c2e6, 0.24),
        track: StyleTone(0xe8f1fc, 0.14),
        text: hex(0xe8f1fc),
        muted: hex(0xa9c2e6),
        faint: hex(0x5a6dab),
        ok: hex(0x8fe0c0),
        warn: hex(0xf2c46d),
        bad: hex(0xff7a8a),
        info: hex(0x8bb3eb),
        receive: hex(0x8bb3eb),
        send: hex(0xd0a6f0),
        clock: hex(0xeef5ff),
        date: hex(0xa9c2e6),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 12.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Yaoguang among theater masks (2026): indigo glass over the masks at the right,
/// mask orange, and one gold line.
pub const YAOGUANG_MASKS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x1c1e43),
        scrim: StyleTone(0x1c1e43, 0.08),
        shade: StyleTone(0x1c1e43, 0.55),
        panel: StyleTone(0x1a1838, 0.74),
        panel_border: StyleTone(0xc2b6c8, 0.24),
        track: StyleTone(0xf4eef2, 0.14),
        text: hex(0xf4eef2),
        muted: hex(0xc2b6c8),
        faint: hex(0x7a6a86),
        ok: hex(0x8fe0c0),
        warn: hex(0xe8c84a),
        bad: hex(0xff6a5a),
        info: hex(0xf0a060),
        receive: hex(0xf0a060),
        send: hex(0xe8c84a),
        clock: hex(0xfff4ec),
        date: hex(0xf0a060),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 0.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Jingliu before the full moon: navy glass over the clouds at both edges, moon
/// white, and sword blue.
pub const JINGLIU_MOON: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x0d1642),
        scrim: StyleTone(0x0d1642, 0.08),
        shade: StyleTone(0x0d1642, 0.55),
        panel: StyleTone(0x0e1640, 0.74),
        panel_border: StyleTone(0xa0b3d6, 0.24),
        track: StyleTone(0xeef1f6, 0.14),
        text: hex(0xeef1f6),
        muted: hex(0xa0b3d6),
        faint: hex(0x5f70b0),
        ok: hex(0x8fe0c0),
        warn: hex(0xf2c46d),
        bad: hex(0xff6b78),
        info: hex(0x7fd0ff),
        receive: hex(0x7fd0ff),
        send: hex(0xc8d4ff),
        clock: hex(0xffffff),
        date: hex(0xa0b3d6),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 8.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.24]),
};

/// Lumine and Aether among constellations: night blue glass at both edges, star
/// gold, and petal white.
pub const LUMINE_STARS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x222346),
        scrim: StyleTone(0x222346, 0.08),
        shade: StyleTone(0x222346, 0.55),
        panel: StyleTone(0x1c2246, 0.74),
        panel_border: StyleTone(0xc9d5eb, 0.24),
        track: StyleTone(0xf8f4f0, 0.14),
        text: hex(0xf8f4f0),
        muted: hex(0xc9d5eb),
        faint: hex(0x6a7fae),
        ok: hex(0x9fe0b0),
        warn: hex(0xf2d28a),
        bad: hex(0xff7a7a),
        info: hex(0xf2d28a),
        receive: hex(0xf2d28a),
        send: hex(0x9fc0ff),
        clock: hex(0xfff8ee),
        date: hex(0xc9d5eb),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 10.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.24]),
};

/// March 7th under meteors: plum glass over the pink sky at the right, rose and
/// periwinkle.
pub const MARCH_METEOR: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x4b3c65),
        scrim: StyleTone(0x4b3c65, 0.08),
        shade: StyleTone(0x4b3c65, 0.55),
        panel: StyleTone(0x352a4c, 0.74),
        panel_border: StyleTone(0xd8c2dc, 0.24),
        track: StyleTone(0xfbf1f8, 0.14),
        text: hex(0xfbf1f8),
        muted: hex(0xd8c2dc),
        faint: hex(0x8f7aa4),
        ok: hex(0x8fe0c0),
        warn: hex(0xf5c26b),
        bad: hex(0xff6b8b),
        info: hex(0x8fd0ff),
        receive: hex(0x8fd0ff),
        send: hex(0xf2a0c8),
        clock: hex(0xfff6fb),
        date: hex(0xf2a0c8),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 14.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Fu Xuan among dark orbits: dusk glass over space at both edges, orchid type,
/// and ring gold.
pub const FU_XUAN_ORBIT: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x231d32),
        scrim: StyleTone(0x231d32, 0.08),
        shade: StyleTone(0x231d32, 0.55),
        panel: StyleTone(0x211a30, 0.74),
        panel_border: StyleTone(0xdcc0d1, 0.24),
        track: StyleTone(0xf6eef6, 0.14),
        text: hex(0xf6eef6),
        muted: hex(0xdcc0d1),
        faint: hex(0x8e7a9c),
        ok: hex(0x8fe0c0),
        warn: hex(0xe8c86a),
        bad: hex(0xff6b8b),
        info: hex(0xe8c86a),
        receive: hex(0xe8c86a),
        send: hex(0xe08ad8),
        clock: hex(0xfff6fb),
        date: hex(0xdcc0d1),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 8.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.22]),
};

/// Firefly in a night meadow: moss glass over the dark grass at both edges, and
/// firefly light.
pub const FIREFLY_MEADOW: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x181822),
        scrim: StyleTone(0x181822, 0.08),
        shade: StyleTone(0x181822, 0.55),
        panel: StyleTone(0x141a18, 0.74),
        panel_border: StyleTone(0xb8c8a8, 0.24),
        track: StyleTone(0xeef2e6, 0.14),
        text: hex(0xeef2e6),
        muted: hex(0xb8c8a8),
        faint: hex(0x6a7a62),
        ok: hex(0xb6e36a),
        warn: hex(0xe8d07a),
        bad: hex(0xff7a6a),
        info: hex(0xe8f0a0),
        receive: hex(0xe8f0a0),
        send: hex(0x9fd0c0),
        clock: hex(0xf6f8ea),
        date: hex(0xb8c8a8),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 12.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.25]),
};

/// Mualani under a Natlan sky: sky cards at the left, deep sea type, and surf
/// teal.
pub const MUALANI_SKY: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x7fabdf),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xf4f9ff, 0.32),
        panel: StyleTone(0xf4f9ff, 0.84),
        panel_border: StyleTone(0x182a4a, 0.16),
        track: StyleTone(0x182a4a, 0.12),
        text: hex(0x182a4a),
        muted: hex(0x4a628c),
        faint: hex(0x98aac8),
        ok: hex(0x2f8a5a),
        warn: hex(0xa86a10),
        bad: hex(0xc0392b),
        info: hex(0x1f8fb0),
        receive: hex(0x1f8fb0),
        send: hex(0xe0704a),
        clock: hex(0x1a3a6a),
        date: hex(0x4a628c),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 16.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Ayaka in frost and butterflies: ice cards over the pale right, indigo type,
/// and her ribbon red.
pub const AYAKA_FROST: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x9fb7e9),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xf5f9ff, 0.32),
        panel: StyleTone(0xf5f9ff, 0.84),
        panel_border: StyleTone(0x1e2a5a, 0.16),
        track: StyleTone(0x1e2a5a, 0.12),
        text: hex(0x1e2a5a),
        muted: hex(0x4f5f96),
        faint: hex(0x9aa8cc),
        ok: hex(0x2f8a5a),
        warn: hex(0xa86a10),
        bad: hex(0xc8405a),
        info: hex(0x3a5ac0),
        receive: hex(0x3a5ac0),
        send: hex(0xc8405a),
        clock: hex(0x26306a),
        date: hex(0x4f5f96),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 12.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Robin with a gift in the snow: navy glass over the pines at the left, snow
/// white, and scarf violet.
pub const ROBIN_SNOW: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x283660),
        scrim: StyleTone(0x283660, 0.08),
        shade: StyleTone(0x283660, 0.55),
        panel: StyleTone(0x1e2850, 0.74),
        panel_border: StyleTone(0xbec2d7, 0.24),
        track: StyleTone(0xf0f2f5, 0.14),
        text: hex(0xf0f2f5),
        muted: hex(0xbec2d7),
        faint: hex(0x7d88b0),
        ok: hex(0x8fe0c0),
        warn: hex(0xf2c46d),
        bad: hex(0xff6a6a),
        info: hex(0x9fb8ff),
        receive: hex(0x9fb8ff),
        send: hex(0xe06a6a),
        clock: hex(0xffffff),
        date: hex(0xbec2d7),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 10.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// March 7th and Stelle at the fireworks: warm night glass at both edges,
/// spark gold, and lilac.
pub const MARCH_FIREWORKS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x292d39),
        scrim: StyleTone(0x292d39, 0.08),
        shade: StyleTone(0x292d39, 0.55),
        panel: StyleTone(0x24242e, 0.74),
        panel_border: StyleTone(0xe0beac, 0.24),
        track: StyleTone(0xfbf1ea, 0.14),
        text: hex(0xfbf1ea),
        muted: hex(0xe0beac),
        faint: hex(0x978786),
        ok: hex(0xa6d6a0),
        warn: hex(0xf2d27a),
        bad: hex(0xff7a6a),
        info: hex(0xf2d27a),
        receive: hex(0xf2d27a),
        send: hex(0xc8a8f0),
        clock: hex(0xfff6ee),
        date: hex(0xe0beac),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 10.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.22]),
};

/// Firefly at a city sunrise: amber cards over the towers at the right, leaf
/// green, and sun gold.
pub const FIREFLY_SUNRISE: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x4e3a30),
        scrim: StyleTone(0x4e3a30, 0.08),
        shade: StyleTone(0x4e3a30, 0.55),
        panel: StyleTone(0x3a2a24, 0.74),
        panel_border: StyleTone(0xf3d4b0, 0.24),
        track: StyleTone(0xfff4e6, 0.14),
        text: hex(0xfff4e6),
        muted: hex(0xf3d4b0),
        faint: hex(0xa8846a),
        ok: hex(0x8fd0a0),
        warn: hex(0xf3d495),
        bad: hex(0xff7a6a),
        info: hex(0xf3d495),
        receive: hex(0xf3d495),
        send: hex(0x8fd0a0),
        clock: hex(0xfff8ec),
        date: hex(0xf3d495),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 8.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};

/// Firefly at violet dusk: lilac glass at both edges, window gold, and sea
/// green.
pub const FIREFLY_DUSK: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x3e355f),
        scrim: StyleTone(0x3e355f, 0.08),
        shade: StyleTone(0x3e355f, 0.55),
        panel: StyleTone(0x2e2850, 0.74),
        panel_border: StyleTone(0xcdb6eb, 0.24),
        track: StyleTone(0xf8f2ff, 0.14),
        text: hex(0xf8f2ff),
        muted: hex(0xcdb6eb),
        faint: hex(0x8a7aa8),
        ok: hex(0x7fd0c0),
        warn: hex(0xf2d28a),
        bad: hex(0xff7a8a),
        info: hex(0xf2d28a),
        receive: hex(0xf2d28a),
        send: hex(0x7fd0c0),
        clock: hex(0xfff8ff),
        date: hex(0xcdb6eb),
    },
    kind: StyleType {
        clock_family: STATUS_SANS_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        title_semibold: true,
        radius: 14.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.23, 0.24]),
};

/// Castorice among white butterflies: slate glass at both edges, wing white,
/// and a violet accent.
pub const CASTORICE_BUTTERFLIES: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x363d51),
        scrim: StyleTone(0x363d51, 0.08),
        shade: StyleTone(0x363d51, 0.55),
        panel: StyleTone(0x2a3044, 0.74),
        panel_border: StyleTone(0xaabcd6, 0.24),
        track: StyleTone(0xeef2f8, 0.14),
        text: hex(0xeef2f8),
        muted: hex(0xaabcd6),
        faint: hex(0x6e7c98),
        ok: hex(0x8fe0c0),
        warn: hex(0xf2c46d),
        bad: hex(0xff6b8b),
        info: hex(0xc6a6ff),
        receive: hex(0xc6a6ff),
        send: hex(0xe8a8d0),
        clock: hex(0xffffff),
        date: hex(0xaabcd6),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 12.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.24, 0.24]),
};

/// Castorice in moonlit ruins: flat violet glass over the sky at the left, moon
/// white, and square corners as the pixel style.
pub const CASTORICE_RUINS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: hex(0x383965),
        scrim: StyleTone(0x383965, 0.08),
        shade: StyleTone(0x383965, 0.55),
        panel: StyleTone(0x30315a, 0.74),
        panel_border: StyleTone(0xc4c0e6, 0.24),
        track: StyleTone(0xf2f0ff, 0.14),
        text: hex(0xf2f0ff),
        muted: hex(0xc4c0e6),
        faint: hex(0x8a85ba),
        ok: hex(0x9fe0c0),
        warn: hex(0xf2d08a),
        bad: hex(0xff7a8a),
        info: hex(0xe8e6ff),
        receive: hex(0xe8e6ff),
        send: hex(0xf0a8c8),
        clock: hex(0xffffff),
        date: hex(0xc4c0e6),
    },
    kind: StyleType {
        clock_family: STATUS_MONO_FONT_FAMILY,
        clock_light: false,
        clock_size: 112.0,
        title_family: STATUS_MONO_FONT_FAMILY,
        title_semibold: true,
        radius: 0.0,
    },
    arrangement: corner_arrangement(StyleAnchor::Top, PanelGroup::Hosts, [0.25, 0.27]),
};
