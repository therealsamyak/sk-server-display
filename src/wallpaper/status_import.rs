//! Local wallpaper identity and editable display settings.

use std::{borrow::Cow, num::NonZeroU32, path::PathBuf};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use super::status_style::{
    NO_AREA, PanelGroup, StatusStyle, StatusStyleName, StyleAnchor, StyleArea, StyleArrangement,
    StyleTone, status_style_by_name,
};

/// A saved wallpaper selection. Imported IDs remain stable when the name changes.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum WallpaperId {
    Preset(StatusStyleName),
    Imported(NonZeroU32),
}

impl WallpaperId {
    /// Select the neutral background with no image.
    pub const PLAIN: Self = Self::Preset(StatusStyleName::Plain);

    /// A stable element ID for a wallpaper card or slideshow row.
    pub fn wallpaper_key(self) -> Cow<'static, str> {
        match self {
            Self::Preset(preset) => Cow::Borrowed(preset.style_name_text()),
            Self::Imported(id) => Cow::Owned(format!("imported-{id}")),
        }
    }

    /// A display name for a built-in background.
    pub fn wallpaper_title(self) -> String {
        if self == Self::PLAIN {
            return "Solid background".into();
        }
        self.wallpaper_key()
            .split('-')
            .map(|word| {
                let mut letters = word.chars();
                let mut title = String::with_capacity(word.len());
                if let Some(first) = letters.next() {
                    title.extend(first.to_uppercase());
                }
                title.push_str(letters.as_str());
                title
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// A user-owned file and the layout that follows it through slideshows.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportedWallpaper {
    pub id: NonZeroU32,
    pub name: String,
    pub path: PathBuf,
    pub placement: PanelPlacement,
    pub appearance: PanelAppearance,
}

impl ImportedWallpaper {
    /// Allocate the next stable import ID.
    pub(super) fn next_import_id(imports: &[Self]) -> anyhow::Result<NonZeroU32> {
        let last = imports
            .iter()
            .map(|entry| entry.id.get())
            .max()
            .unwrap_or(0);
        last.checked_add(1)
            .and_then(NonZeroU32::new)
            .context("wallpaper collection has no free IDs")
    }

    /// Validate fields before saving this wallpaper.
    pub(super) fn validate_import(&self) -> anyhow::Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.chars().count() <= 80,
            "enter a wallpaper name from 1 to 80 characters"
        );
        ensure!(self.path.is_absolute(), "enter an absolute image path");
        Ok(())
    }

    /// Derive the complete display style for this image.
    pub fn wallpaper_style(&self) -> StatusStyle {
        let mut style = *status_style_by_name(StatusStyleName::Plain);
        let (scrim, panel, shade) = match self.appearance {
            PanelAppearance::Glass => (0.32, 0.78, 0.20),
            PanelAppearance::Opaque => (0.48, 1.0, 0.30),
        };
        style.palette.scrim = StyleTone(0, scrim);
        style.palette.shade = StyleTone(0, shade);
        style.palette.panel.1 = panel;
        style.arrangement = self.placement.wallpaper_arrangement();
        style
    }
}

/// Panel and clock placement relative to the image.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PanelPlacement {
    #[default]
    Bottom,
    Left,
    Right,
    Split,
}

impl PanelPlacement {
    fn wallpaper_arrangement(self) -> StyleArrangement {
        const SIDE: StyleArea = StyleArea {
            width: 0.56,
            clock: Some(StyleAnchor::Top),
            groups: &[PanelGroup::Hosts, PanelGroup::Others],
            panels: StyleAnchor::Bottom,
        };
        match self {
            Self::Bottom => StyleArrangement::Band,
            Self::Left => StyleArrangement::Sides {
                left: SIDE,
                right: NO_AREA,
            },
            Self::Right => StyleArrangement::Sides {
                left: NO_AREA,
                right: SIDE,
            },
            Self::Split => StyleArrangement::Sides {
                left: StyleArea {
                    width: 0.28,
                    groups: &[PanelGroup::Hosts],
                    ..SIDE
                },
                right: StyleArea {
                    width: 0.30,
                    clock: None,
                    groups: &[PanelGroup::Others],
                    ..SIDE
                },
            },
        }
    }
}

/// Contrast and opacity of the wallpaper scrim and metric panels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PanelAppearance {
    #[default]
    Glass,
    Opaque,
}
