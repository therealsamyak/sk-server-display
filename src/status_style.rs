//! Status styles: the palette, fonts, and panel placement of one wallpaper.
//!
//! Each style follows one wallpaper. Its panels sit where the art is quiet,
//! and its colors come from the art. The configuration key `style` selects
//! one style for the whole run.

use std::sync::OnceLock;

use gpui::{Hsla, rgb};
use serde::Deserialize;

use crate::status_fonts::{
    STATUS_MONO_FONT_FAMILY, STATUS_SANS_FONT_FAMILY, STATUS_SERIF_FONT_FAMILY,
};
use crate::status_style_contest;
use crate::status_style_winners;

/// Name of a built-in status style, as the configuration key `style` gives it.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StatusStyleName {
    /// Clock at the top and a panel band at the bottom, for a landscape.
    #[default]
    Hills,
    YolandaKokomi,
    YolandaNilou,
    YolandaRaiden,
    YolandaAcheron,
    YolandaNahida,
    YolandaFugue,
    YolandaAglaea,
    YolandaEvernight,
    YolandaYaoguang,
    FurinaBalloon,
    RosariaRuins,
    LumineCathedral,
    AyakaMoonlit,
    KeqingWedding,
    BronyaSnow,
    FuXuanStars,
    NahidaForest,
    FocalorsBlossom,
    KiraraDelivery,
    FurinaSky,
    RuanMeiBlossom,
    MavuikaRide,
    CastoriceGarden,
    YaeMist,
    ShenheLanterns,
    FurinaDeep,
    KafkaRuins,
    MarchShrine,
    AcheronRain,
    OdetteStars,
    YaoguangMasks,
    JingliuMoon,
    LumineStars,
    MarchMeteor,
    FuXuanOrbit,
    FireflyMeadow,
    MualaniSky,
    AyakaFrost,
    RobinSnow,
    MarchFireworks,
    FireflySunrise,
    FireflyDusk,
    CastoriceButterflies,
    CastoriceRuins,
    TrailblazersOrbit,
    CyreneBow,
    PearlWater,
    StelleShards,
    SparxieStage,
    KafkaCity,
    AglaeaMeadow,
    CastoriceViolet,
    CyreneFlowers,
    RobinConcert,
    SparkleMaple,
    AcheronEclipse,
    HuntersDaylight,
    LumineLinen,
}

/// One color with its opacity.
#[derive(Clone, Copy, Debug)]
pub struct StyleTone(pub u32, pub f32);

impl StyleTone {
    /// GPUI color of this tone.
    pub fn status_color(self) -> Hsla {
        Hsla::from(rgb(self.0)).opacity(self.1)
    }
}

/// Opaque tone from a hex color.
const fn solid(hex: u32) -> StyleTone {
    StyleTone(hex, 1.0)
}

/// Colors of one status style.
#[derive(Clone, Copy, Debug)]
pub struct StylePalette {
    /// Window color without a wallpaper, and while the wallpaper loads.
    pub background: StyleTone,
    /// Layer over the whole wallpaper.
    pub scrim: StyleTone,
    /// Layer behind each panel area. It fades out toward the art.
    pub shade: StyleTone,
    pub panel: StyleTone,
    pub panel_border: StyleTone,
    pub track: StyleTone,
    pub text: StyleTone,
    pub muted: StyleTone,
    pub faint: StyleTone,
    pub ok: StyleTone,
    pub warn: StyleTone,
    pub bad: StyleTone,
    pub info: StyleTone,
    pub receive: StyleTone,
    pub send: StyleTone,
    pub clock: StyleTone,
    pub date: StyleTone,
}

/// Fonts and size of the clock, and the font of the panel titles.
#[derive(Clone, Copy, Debug)]
pub struct StyleType {
    pub clock_family: &'static str,
    /// `true` gives the light weight. Plex Mono has no light weight.
    pub clock_light: bool,
    /// Clock text size, in design pixels.
    pub clock_size: f32,
    pub title_family: &'static str,
    /// `true` gives the semibold weight to the panel titles.
    pub title_semibold: bool,
    /// Panel corner radius, in design pixels.
    pub radius: f32,
}

/// Vertical place of a block in a panel area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleAnchor {
    Top,
    Bottom,
}

/// Panel group: the host cards, or the services, disks, and tailnet panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelGroup {
    Hosts,
    Others,
}

/// One side area of the screen: a column at the left or the right edge.
#[derive(Clone, Copy, Debug)]
pub struct StyleArea {
    /// Part of the window width, from 0 to 1. Zero: no area.
    pub width: f32,
    /// Place of the clock and the alerts. `None`: the clock is not here.
    pub clock: Option<StyleAnchor>,
    /// Panel groups, one column each, from the outer edge inward.
    pub groups: &'static [PanelGroup],
    /// Place of the panel columns.
    pub panels: StyleAnchor,
}

/// An empty side area.
const NO_AREA: StyleArea = StyleArea {
    width: 0.0,
    clock: None,
    groups: &[],
    panels: StyleAnchor::Top,
};

/// Where the clock and the panels go.
#[derive(Clone, Copy, Debug)]
pub enum StyleArrangement {
    /// Clock at the top, all panels in a band at the bottom.
    Band,
    /// A column at the left edge and one at the right edge. The middle
    /// stays free for the art.
    Sides { left: StyleArea, right: StyleArea },
}

/// One status style: palette, type, and arrangement.
#[derive(Clone, Copy, Debug)]
pub struct StatusStyle {
    pub palette: StylePalette,
    pub kind: StyleType,
    pub arrangement: StyleArrangement,
}

static ACTIVE_STATUS_STYLE: OnceLock<&'static StatusStyle> = OnceLock::new();

/// Selects the status style for this run. Call it once, before the first
/// window opens.
pub fn set_status_style(name: StatusStyleName) {
    if ACTIVE_STATUS_STYLE.set(status_style_by_name(name)).is_err() {
        log::warn!("status style is already set");
    }
}

/// The status style of this run. Without `set_status_style`: `hills`.
pub fn active_status_style() -> &'static StatusStyle {
    ACTIVE_STATUS_STYLE.get_or_init(|| status_style_by_name(StatusStyleName::Hills))
}

/// Built-in status style with this name.
pub fn status_style_by_name(name: StatusStyleName) -> &'static StatusStyle {
    match name {
        StatusStyleName::Hills => &HILLS,
        StatusStyleName::YolandaKokomi => &YOLANDA_KOKOMI,
        StatusStyleName::YolandaNilou => &YOLANDA_NILOU,
        StatusStyleName::YolandaRaiden => &YOLANDA_RAIDEN,
        StatusStyleName::YolandaAcheron => &YOLANDA_ACHERON,
        StatusStyleName::YolandaNahida => &YOLANDA_NAHIDA,
        StatusStyleName::YolandaFugue => &YOLANDA_FUGUE,
        StatusStyleName::YolandaAglaea => &YOLANDA_AGLAEA,
        StatusStyleName::YolandaEvernight => &YOLANDA_EVERNIGHT,
        StatusStyleName::YolandaYaoguang => &YOLANDA_YAOGUANG,
        StatusStyleName::FurinaBalloon => &status_style_contest::FURINA_BALLOON,
        StatusStyleName::RosariaRuins => &status_style_contest::ROSARIA_RUINS,
        StatusStyleName::LumineCathedral => &status_style_contest::LUMINE_CATHEDRAL,
        StatusStyleName::AyakaMoonlit => &status_style_contest::AYAKA_MOONLIT,
        StatusStyleName::KeqingWedding => &status_style_contest::KEQING_WEDDING,
        StatusStyleName::BronyaSnow => &status_style_contest::BRONYA_SNOW,
        StatusStyleName::FuXuanStars => &status_style_contest::FU_XUAN_STARS,
        StatusStyleName::NahidaForest => &status_style_contest::NAHIDA_FOREST,
        StatusStyleName::FocalorsBlossom => &status_style_contest::FOCALORS_BLOSSOM,
        StatusStyleName::KiraraDelivery => &status_style_contest::KIRARA_DELIVERY,
        StatusStyleName::FurinaSky => &status_style_contest::FURINA_SKY,
        StatusStyleName::RuanMeiBlossom => &status_style_contest::RUAN_MEI_BLOSSOM,
        StatusStyleName::MavuikaRide => &status_style_contest::MAVUIKA_RIDE,
        StatusStyleName::CastoriceGarden => &status_style_contest::CASTORICE_GARDEN,
        StatusStyleName::YaeMist => &status_style_contest::YAE_MIST,
        StatusStyleName::ShenheLanterns => &status_style_contest::SHENHE_LANTERNS,
        StatusStyleName::FurinaDeep => &status_style_contest::FURINA_DEEP,
        StatusStyleName::KafkaRuins => &status_style_contest::KAFKA_RUINS,
        StatusStyleName::MarchShrine => &status_style_contest::MARCH_SHRINE,
        StatusStyleName::AcheronRain => &status_style_contest::ACHERON_RAIN,
        StatusStyleName::OdetteStars => &status_style_contest::ODETTE_STARS,
        StatusStyleName::YaoguangMasks => &status_style_contest::YAOGUANG_MASKS,
        StatusStyleName::JingliuMoon => &status_style_contest::JINGLIU_MOON,
        StatusStyleName::LumineStars => &status_style_contest::LUMINE_STARS,
        StatusStyleName::MarchMeteor => &status_style_contest::MARCH_METEOR,
        StatusStyleName::FuXuanOrbit => &status_style_contest::FU_XUAN_ORBIT,
        StatusStyleName::FireflyMeadow => &status_style_contest::FIREFLY_MEADOW,
        StatusStyleName::MualaniSky => &status_style_contest::MUALANI_SKY,
        StatusStyleName::AyakaFrost => &status_style_contest::AYAKA_FROST,
        StatusStyleName::RobinSnow => &status_style_contest::ROBIN_SNOW,
        StatusStyleName::MarchFireworks => &status_style_contest::MARCH_FIREWORKS,
        StatusStyleName::FireflySunrise => &status_style_contest::FIREFLY_SUNRISE,
        StatusStyleName::FireflyDusk => &status_style_contest::FIREFLY_DUSK,
        StatusStyleName::CastoriceButterflies => &status_style_contest::CASTORICE_BUTTERFLIES,
        StatusStyleName::CastoriceRuins => &status_style_contest::CASTORICE_RUINS,
        StatusStyleName::TrailblazersOrbit => &status_style_winners::TRAILBLAZERS_ORBIT,
        StatusStyleName::CyreneBow => &status_style_winners::CYRENE_BOW,
        StatusStyleName::PearlWater => &status_style_winners::PEARL_WATER,
        StatusStyleName::StelleShards => &status_style_winners::STELLE_SHARDS,
        StatusStyleName::SparxieStage => &status_style_winners::SPARXIE_STAGE,
        StatusStyleName::KafkaCity => &status_style_winners::KAFKA_CITY,
        StatusStyleName::AglaeaMeadow => &status_style_winners::AGLAEA_MEADOW,
        StatusStyleName::CastoriceViolet => &status_style_winners::CASTORICE_VIOLET,
        StatusStyleName::CyreneFlowers => &status_style_winners::CYRENE_FLOWERS,
        StatusStyleName::RobinConcert => &status_style_winners::ROBIN_CONCERT,
        StatusStyleName::SparkleMaple => &status_style_winners::SPARKLE_MAPLE,
        StatusStyleName::AcheronEclipse => &status_style_winners::ACHERON_ECLIPSE,
        StatusStyleName::HuntersDaylight => &status_style_winners::HUNTERS_DAYLIGHT,
        StatusStyleName::LumineLinen => &status_style_winners::LUMINE_LINEN,
    }
}

const SANS_TYPE: StyleType = StyleType {
    clock_family: STATUS_SANS_FONT_FAMILY,
    clock_light: true,
    clock_size: 128.0,
    title_family: STATUS_MONO_FONT_FAMILY,
    title_semibold: true,
    radius: 8.0,
};

/// Default style: the landscape wallpaper with a panel band.
const HILLS: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x10151c),
        scrim: StyleTone(0x000000, 0.28),
        shade: StyleTone(0x000000, 0.0),
        panel: StyleTone(0x0c1016, 0.78),
        panel_border: StyleTone(0xffffff, 0.10),
        track: StyleTone(0xffffff, 0.10),
        text: solid(0xeef1f4),
        muted: solid(0x9aa4af),
        faint: solid(0x5f6873),
        ok: solid(0x3fb950),
        warn: solid(0xd29922),
        bad: solid(0xf85149),
        info: solid(0x58a6ff),
        receive: solid(0x58a6ff),
        send: solid(0xbc8cff),
        clock: solid(0xffffff),
        date: solid(0xffffff),
    },
    kind: SANS_TYPE,
    arrangement: StyleArrangement::Band,
};

/// Kokomi among jellyfish: navy glass rails at both dark edges.
const YOLANDA_KOKOMI: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x060f33),
        scrim: StyleTone(0x010c2b, 0.08),
        shade: StyleTone(0x010c2b, 0.55),
        panel: StyleTone(0x0a1540, 0.76),
        panel_border: StyleTone(0x8290d0, 0.24),
        track: StyleTone(0xc2c8ea, 0.14),
        text: solid(0xe8ecff),
        muted: solid(0xa6b0dc),
        faint: solid(0x6a75a8),
        ok: solid(0x6fd6c0),
        warn: solid(0xf2c46d),
        bad: solid(0xff6b8a),
        info: solid(0x8fb4ff),
        receive: solid(0x7fc8ff),
        send: solid(0xf3b6d0),
        clock: solid(0xeef1ff),
        date: solid(0xc2c8ea),
    },
    kind: StyleType {
        title_family: STATUS_SANS_FONT_FAMILY,
        radius: 12.0,
        ..SANS_TYPE
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.25,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.25,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Top,
        },
    },
};

/// Nilou in a gold mandala: serif gold type and thin gold frames.
const YOLANDA_NILOU: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x0e0e1d),
        scrim: StyleTone(0x030316, 0.10),
        shade: StyleTone(0x030316, 0.60),
        panel: StyleTone(0x0b0b1a, 0.72),
        panel_border: StyleTone(0xd0b99a, 0.36),
        track: StyleTone(0xd0b99a, 0.16),
        text: solid(0xf4e9d8),
        muted: solid(0xc4ab8c),
        faint: solid(0x7d6a58),
        ok: solid(0x7cc9a6),
        warn: solid(0xf0b85a),
        bad: solid(0xff6a5c),
        info: solid(0x5fb8c8),
        receive: solid(0x5fb8c8),
        send: solid(0xe8875a),
        clock: solid(0xe9c88a),
        date: solid(0xd0b99a),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 120.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 2.0,
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.24,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.24,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Raiden Shogun and Ei: the clock and the hosts over the dark starfield,
/// the services over the city at the right.
const YOLANDA_RAIDEN: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x06041e),
        scrim: StyleTone(0x06041e, 0.10),
        shade: StyleTone(0x06041e, 0.62),
        panel: StyleTone(0x15123d, 0.68),
        panel_border: StyleTone(0x8d78b1, 0.20),
        track: StyleTone(0xd4c2e7, 0.14),
        text: solid(0xf1ebff),
        muted: solid(0xb9a8dc),
        faint: solid(0x6f6296),
        ok: solid(0x8be0c0),
        warn: solid(0xf5c26b),
        bad: solid(0xff6b8b),
        info: solid(0xb48cff),
        receive: solid(0x9fb8ff),
        send: solid(0xd4a2ff),
        clock: solid(0xf1ebff),
        date: solid(0xd4c2e7),
    },
    kind: StyleType {
        clock_size: 112.0,
        title_family: STATUS_SANS_FONT_FAMILY,
        radius: 6.0,
        ..SANS_TYPE
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.23,
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

/// Acheron and Black Swan: black, white, and one red. Mono type, as the
/// signature on the art.
const YOLANDA_ACHERON: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x080304),
        scrim: StyleTone(0x000000, 0.12),
        shade: StyleTone(0x080304, 0.60),
        panel: StyleTone(0x0a0708, 0.82),
        panel_border: StyleTone(0xffffff, 0.12),
        track: StyleTone(0xffffff, 0.12),
        text: solid(0xf2f0f2),
        muted: solid(0xa09ba8),
        faint: solid(0x625c66),
        ok: solid(0xe9e8e9),
        warn: solid(0xd9a441),
        bad: solid(0xe5383b),
        info: solid(0xa09ba8),
        receive: solid(0xc9c4d0),
        send: solid(0x8f7fa8),
        clock: solid(0xffffff),
        date: solid(0xe5383b),
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
            width: 0.25,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts],
            panels: StyleAnchor::Bottom,
        },
        right: StyleArea {
            width: 0.25,
            clock: None,
            groups: &[PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        },
    },
};

/// Nahida in a flower ring: a light style. Pale cards over the flowers at
/// the left, the clock under them. Nahida and her friends stay free.
const YOLANDA_NAHIDA: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x6ab9cc),
        scrim: StyleTone(0xffffff, 0.0),
        shade: StyleTone(0xf4fbf2, 0.30),
        panel: StyleTone(0xf4fbf2, 0.84),
        panel_border: StyleTone(0x2f5d45, 0.16),
        track: StyleTone(0x17352b, 0.12),
        text: solid(0x17352b),
        muted: solid(0x4b6b5c),
        faint: solid(0x8aa396),
        ok: solid(0x2e9b57),
        warn: solid(0xb87400),
        bad: solid(0xd0393b),
        info: solid(0x2a86b0),
        receive: solid(0x2a86b0),
        send: solid(0xc0548a),
        clock: solid(0x17402f),
        date: solid(0x17402f),
    },
    kind: StyleType {
        title_family: STATUS_SANS_FONT_FAMILY,
        clock_size: 112.0,
        radius: 16.0,
        ..SANS_TYPE
    },
    arrangement: StyleArrangement::Sides {
        left: StyleArea {
            width: 0.44,
            clock: Some(StyleAnchor::Bottom),
            groups: &[PanelGroup::Hosts, PanelGroup::Others],
            panels: StyleAnchor::Top,
        },
        right: NO_AREA,
    },
};

/// Fugue as a fox spirit at night: warm cards over the dark tails at the
/// left. The moon and its light stay free at the right.
const YOLANDA_FUGUE: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x19141b),
        scrim: StyleTone(0x000000, 0.08),
        shade: StyleTone(0x120d12, 0.62),
        panel: StyleTone(0x1c1418, 0.72),
        panel_border: StyleTone(0xf3cca6, 0.16),
        track: StyleTone(0xf3cca6, 0.12),
        text: solid(0xfbeee2),
        muted: solid(0xc9a998),
        faint: solid(0x7e6660),
        ok: solid(0x9ccf7a),
        warn: solid(0xf3b04a),
        bad: solid(0xff6a55),
        info: solid(0xf3cca6),
        receive: solid(0xf3cca6),
        send: solid(0xe0604a),
        clock: solid(0xfbe3c6),
        date: solid(0xf3cca6),
    },
    kind: StyleType {
        title_family: STATUS_SANS_FONT_FAMILY,
        clock_size: 112.0,
        radius: 10.0,
        ..SANS_TYPE
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

/// Aglaea in a cathedral: the gold window stays free in the middle. Serif
/// type and dark warm cards over the candles at both sides.
const YOLANDA_AGLAEA: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x1a1513),
        scrim: StyleTone(0x000000, 0.06),
        shade: StyleTone(0x140f0b, 0.58),
        panel: StyleTone(0x17110c, 0.70),
        panel_border: StyleTone(0xdec8ac, 0.22),
        track: StyleTone(0xdec8ac, 0.14),
        text: solid(0xfdf7e1),
        muted: solid(0xcdb89c),
        faint: solid(0x85735e),
        ok: solid(0xb5d48a),
        warn: solid(0xf2c14e),
        bad: solid(0xff7a5c),
        info: solid(0xe8cf9a),
        receive: solid(0xf6dfa6),
        send: solid(0xb8c4d6),
        clock: solid(0xfdf3d6),
        date: solid(0xdec8ac),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 128.0,
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

/// Evernight among letters: pale paper cards and dark red type, as the
/// notes on the red carpet.
const YOLANDA_EVERNIGHT: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x898c9d),
        scrim: StyleTone(0xffffff, 0.04),
        shade: StyleTone(0x1a0909, 0.50),
        panel: StyleTone(0xf3f1f4, 0.88),
        panel_border: StyleTone(0x742a2b, 0.28),
        track: StyleTone(0x2a1418, 0.12),
        text: solid(0x2a1418),
        muted: solid(0x6b5458),
        faint: solid(0xa0939a),
        ok: solid(0x2f8f5b),
        warn: solid(0xb8720a),
        bad: solid(0xb3202e),
        info: solid(0x4a5f9a),
        receive: solid(0x4a5f9a),
        send: solid(0x8a5a9a),
        clock: solid(0xf6eef0),
        date: solid(0xf0c8cc),
    },
    kind: StyleType {
        clock_family: STATUS_SERIF_FONT_FAMILY,
        clock_light: true,
        clock_size: 112.0,
        title_family: STATUS_SERIF_FONT_FAMILY,
        title_semibold: false,
        radius: 2.0,
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
            panels: StyleAnchor::Top,
        },
    },
};

/// Yaoguang with peacock feathers: teal glass and gold type at the left.
const YOLANDA_YAOGUANG: StatusStyle = StatusStyle {
    palette: StylePalette {
        background: solid(0x071b29),
        scrim: StyleTone(0x000000, 0.06),
        shade: StyleTone(0x041620, 0.62),
        panel: StyleTone(0x06212a, 0.68),
        panel_border: StyleTone(0xe5e0bc, 0.18),
        track: StyleTone(0xe5e0bc, 0.12),
        text: solid(0xeef6f0),
        muted: solid(0xa9c2bb),
        faint: solid(0x5f7c78),
        ok: solid(0x6fdcb4),
        warn: solid(0xf2c14e),
        bad: solid(0xff6f7d),
        info: solid(0x5fc4e0),
        receive: solid(0x5fc4e0),
        send: solid(0xf2b84a),
        clock: solid(0xf6e7b8),
        date: solid(0xa9c2bb),
    },
    kind: StyleType {
        title_family: STATUS_SANS_FONT_FAMILY,
        clock_size: 112.0,
        ..SANS_TYPE
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

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_STYLE_NAMES: [StatusStyleName; 59] = [
        StatusStyleName::Hills,
        StatusStyleName::YolandaKokomi,
        StatusStyleName::YolandaNilou,
        StatusStyleName::YolandaRaiden,
        StatusStyleName::YolandaAcheron,
        StatusStyleName::YolandaNahida,
        StatusStyleName::YolandaFugue,
        StatusStyleName::YolandaAglaea,
        StatusStyleName::YolandaEvernight,
        StatusStyleName::YolandaYaoguang,
        StatusStyleName::FurinaBalloon,
        StatusStyleName::RosariaRuins,
        StatusStyleName::LumineCathedral,
        StatusStyleName::AyakaMoonlit,
        StatusStyleName::KeqingWedding,
        StatusStyleName::BronyaSnow,
        StatusStyleName::FuXuanStars,
        StatusStyleName::NahidaForest,
        StatusStyleName::FocalorsBlossom,
        StatusStyleName::KiraraDelivery,
        StatusStyleName::FurinaSky,
        StatusStyleName::RuanMeiBlossom,
        StatusStyleName::MavuikaRide,
        StatusStyleName::CastoriceGarden,
        StatusStyleName::YaeMist,
        StatusStyleName::ShenheLanterns,
        StatusStyleName::FurinaDeep,
        StatusStyleName::KafkaRuins,
        StatusStyleName::MarchShrine,
        StatusStyleName::AcheronRain,
        StatusStyleName::OdetteStars,
        StatusStyleName::YaoguangMasks,
        StatusStyleName::JingliuMoon,
        StatusStyleName::LumineStars,
        StatusStyleName::MarchMeteor,
        StatusStyleName::FuXuanOrbit,
        StatusStyleName::FireflyMeadow,
        StatusStyleName::MualaniSky,
        StatusStyleName::AyakaFrost,
        StatusStyleName::RobinSnow,
        StatusStyleName::MarchFireworks,
        StatusStyleName::FireflySunrise,
        StatusStyleName::FireflyDusk,
        StatusStyleName::CastoriceButterflies,
        StatusStyleName::CastoriceRuins,
        StatusStyleName::TrailblazersOrbit,
        StatusStyleName::CyreneBow,
        StatusStyleName::PearlWater,
        StatusStyleName::StelleShards,
        StatusStyleName::SparxieStage,
        StatusStyleName::KafkaCity,
        StatusStyleName::AglaeaMeadow,
        StatusStyleName::CastoriceViolet,
        StatusStyleName::CyreneFlowers,
        StatusStyleName::RobinConcert,
        StatusStyleName::SparkleMaple,
        StatusStyleName::AcheronEclipse,
        StatusStyleName::HuntersDaylight,
        StatusStyleName::LumineLinen,
    ];

    #[test]
    fn side_styles_place_each_panel_group_and_the_clock_once() {
        for name in ALL_STYLE_NAMES {
            let StyleArrangement::Sides { left, right } = status_style_by_name(name).arrangement
            else {
                continue;
            };
            let areas = [left, right];
            for group in [PanelGroup::Hosts, PanelGroup::Others] {
                let count = areas
                    .iter()
                    .flat_map(|area| area.groups)
                    .filter(|placed| **placed == group)
                    .count();
                assert_eq!(count, 1, "{name:?} places {group:?} {count} times");
            }
            let clocks = areas.iter().filter(|area| area.clock.is_some()).count();
            assert_eq!(clocks, 1, "{name:?} has {clocks} clocks");
            for area in areas {
                assert!(
                    area.width > 0. || (area.groups.is_empty() && area.clock.is_none()),
                    "{name:?} has content in an area without width"
                );
            }
            assert!(
                left.width + right.width <= 0.6,
                "{name:?} covers the middle"
            );
        }
    }

    #[test]
    fn parses_style_names_in_kebab_case() {
        #[derive(Deserialize)]
        struct Holder {
            style: StatusStyleName,
        }
        let holder: Holder = toml::from_str("style = \"yolanda-evernight\"").expect("style parses");
        assert_eq!(holder.style, StatusStyleName::YolandaEvernight);
        assert!(toml::from_str::<Holder>("style = \"unknown\"").is_err());
    }
}
