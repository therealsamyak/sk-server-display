//! Saved slideshow settings and selection for the status display.

use std::{
    collections::{HashMap, HashSet},
    io::{self, Write as _},
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use crate::status_style::StatusStyleName;

/// One ordered set of wallpapers and its time per slide.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StatusPlaylist {
    pub name: String,
    pub styles: Vec<StatusStyleName>,
    pub interval_minutes: NonZeroU32,
}

impl StatusPlaylist {
    /// Time to show each available wallpaper.
    pub fn slide_duration(&self) -> Duration {
        Duration::from_secs(u64::from(self.interval_minutes.get()) * 60)
    }

    /// Select the next available slide after the current one, with wraparound.
    pub fn next_slide(
        &self,
        current: Option<StatusStyleName>,
        available: impl Fn(StatusStyleName) -> bool,
    ) -> Option<StatusStyleName> {
        let start = current
            .and_then(|style| self.styles.iter().position(|item| *item == style))
            .map_or(0, |index| index + 1);
        self.styles[start..]
            .iter()
            .chain(&self.styles[..start])
            .copied()
            .find(|style| available(*style))
    }
}

/// Favorites and the user-created slideshows.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlaylistSettings {
    pub favorites: StatusPlaylist,
    pub slideshows: Vec<StatusPlaylist>,
}

impl Default for PlaylistSettings {
    fn default() -> Self {
        use StatusStyleName::*;
        Self {
            favorites: StatusPlaylist {
                name: "Favorites".into(),
                styles: vec![
                    FurinaDeep,
                    FocalorsBlossom,
                    FuXuanStars,
                    YolandaNahida,
                    YolandaKokomi,
                    TrailblazersOrbit,
                    CyreneBow,
                    CastoriceViolet,
                ],
                interval_minutes: NonZeroU32::new(60).unwrap_or(NonZeroU32::MIN),
            },
            slideshows: Vec::new(),
        }
    }
}

impl PlaylistSettings {
    /// Find the saved slideshow named by this selection.
    pub fn selected_playlist(&self, choice: &PlaylistChoice) -> Option<&StatusPlaylist> {
        match choice {
            PlaylistChoice::Favorites => Some(&self.favorites),
            PlaylistChoice::Custom(name) => self.slideshows.iter().find(|list| &list.name == name),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        let mut names = HashSet::new();
        ensure!(self.favorites.name == "Favorites", "invalid Favorites name");
        for list in std::iter::once(&self.favorites).chain(&self.slideshows) {
            ensure!(
                !list.name.trim().is_empty() && names.insert(list.name.to_lowercase()),
                "slideshow names must be unique and nonempty"
            );
            let styles: HashSet<_> = list.styles.iter().collect();
            ensure!(
                styles.len() == list.styles.len(),
                "a slideshow contains duplicate styles"
            );
        }
        Ok(())
    }
}

/// Favorites or a custom slideshow in the user's collection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlaylistChoice {
    Favorites,
    Custom(String),
}

/// Persistent mode: hold one wallpaper or advance an ordered slideshow.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", content = "selection", rename_all = "kebab-case")]
pub enum StatusPlaybackMode {
    Fixed(StatusStyleName),
    Slideshow(PlaylistChoice),
}

/// Settings saved beside the hand-written server configuration.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StatusPlayback {
    pub settings: PlaylistSettings,
    pub playback: StatusPlaybackMode,
}

impl Default for StatusPlayback {
    fn default() -> Self {
        Self {
            settings: PlaylistSettings::default(),
            playback: StatusPlaybackMode::Slideshow(PlaylistChoice::Favorites),
        }
    }
}

impl StatusPlayback {
    /// Load the saved collection, or use Favorites on first launch.
    pub fn load_status_playback(path: Option<&Path>) -> anyhow::Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).context("cannot read slideshow state"),
        };
        let state: Self = toml::from_str(&text).context("invalid slideshow state")?;
        state.settings.validate()?;
        Ok(state)
    }

    /// Save valid slideshow state in one atomic file replacement.
    pub fn save_status_playback(&self, path: &Path) -> anyhow::Result<()> {
        self.settings.validate()?;
        let contents = toml::to_string_pretty(self).context("cannot serialize slideshow state")?;
        let directory = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(directory).context("cannot create slideshow state folder")?;
        let mut file = tempfile::NamedTempFile::new_in(directory)
            .context("cannot create temporary slideshow state file")?;
        file.write_all(contents.as_bytes())
            .context("cannot write slideshow state")?;
        file.as_file()
            .sync_all()
            .context("cannot sync slideshow state")?;
        file.persist(path)
            .context("cannot replace slideshow state")?;
        Ok(())
    }

    /// Return the current slideshow if the display is in rotation mode.
    pub fn playing_playlist(&self) -> Option<&StatusPlaylist> {
        match &self.playback {
            StatusPlaybackMode::Fixed(_) => None,
            StatusPlaybackMode::Slideshow(choice) => self.settings.selected_playlist(choice),
        }
    }
}

/// Runtime selection and timing shared by replacement status windows.
pub struct StatusCarousel {
    pub state: StatusPlayback,
    pub state_path: Option<PathBuf>,
    pub wallpapers: Arc<HashMap<StatusStyleName, PathBuf>>,
    pub current: StatusStyleName,
    pub last_switch: Instant,
    failed: HashSet<StatusStyleName>,
}

impl StatusCarousel {
    /// Open the saved slideshow at its first available wallpaper.
    pub fn new_status_carousel(
        state: StatusPlayback,
        state_path: Option<PathBuf>,
        wallpapers: HashMap<StatusStyleName, PathBuf>,
    ) -> Self {
        let mut carousel = Self {
            state,
            state_path,
            wallpapers: Arc::new(wallpapers),
            current: StatusStyleName::Hills,
            last_switch: Instant::now(),
            failed: HashSet::new(),
        };
        carousel.select_initial_slide();
        carousel
    }

    /// Select the first available slide for the current playback mode.
    pub fn select_initial_slide(&mut self) {
        self.current = match &self.state.playback {
            StatusPlaybackMode::Fixed(style) if self.available(*style) => *style,
            StatusPlaybackMode::Fixed(_) => StatusStyleName::Hills,
            StatusPlaybackMode::Slideshow(_) => self
                .next_available_slide(None)
                .unwrap_or(StatusStyleName::Hills),
        };
        self.last_switch = Instant::now();
    }

    fn available(&self, style: StatusStyleName) -> bool {
        style == StatusStyleName::Hills || self.wallpaper_path(style).is_some()
    }

    /// The real wallpaper path for a style, if decoding has succeeded or is untried.
    pub fn wallpaper_path(&self, style: StatusStyleName) -> Option<&Path> {
        if self.failed.contains(&style) {
            return None;
        }
        self.wallpapers.get(&style).map(PathBuf::as_path)
    }

    /// Find the next slideshow entry with an available wallpaper.
    pub fn next_available_slide(
        &self,
        current: Option<StatusStyleName>,
    ) -> Option<StatusStyleName> {
        self.state
            .playing_playlist()?
            .next_slide(current, |style| self.available(style))
    }

    /// Exclude bad art from later rotation and return the next usable slide.
    pub fn skip_failed_wallpaper(&mut self, style: StatusStyleName) {
        self.failed.insert(style);
        if style == self.current {
            self.current = self
                .next_available_slide(Some(style))
                .unwrap_or(StatusStyleName::Hills);
            self.last_switch = Instant::now();
        }
    }

    /// Save changes, then update the running slideshow or fixed wallpaper.
    pub fn update_status_playback(&mut self, next: StatusPlayback) -> anyhow::Result<()> {
        let path = self
            .state_path
            .as_deref()
            .context("no slideshow state path")?;
        next.save_status_playback(path)?;
        self.state = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use StatusStyleName::*;

    #[test]
    fn rotation_skips_missing_art_and_wraps_in_order() {
        let list = StatusPlaylist {
            name: "Test".into(),
            styles: vec![CyreneBow, FurinaDeep, YolandaKokomi],
            interval_minutes: NonZeroU32::MIN,
        };
        let available = |style| style != FurinaDeep;
        assert_eq!(
            list.next_slide(Some(CyreneBow), available),
            Some(YolandaKokomi)
        );
        assert_eq!(
            list.next_slide(Some(YolandaKokomi), available),
            Some(CyreneBow)
        );
        assert_eq!(list.next_slide(None, |_| false), None);
    }

    #[test]
    fn round_trip_replaces_saved_selection_and_preserves_long_minute_interval() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("state.toml");
        let mut state = StatusPlayback::load_status_playback(Some(&path)).expect("default state");
        state.save_status_playback(&path).expect("initial save");
        state.settings.slideshows.push(StatusPlaylist {
            name: "Night".into(),
            styles: vec![CyreneBow, YolandaKokomi],
            interval_minutes: NonZeroU32::new(10_081).expect("nonzero"),
        });
        state.playback = StatusPlaybackMode::Slideshow(PlaylistChoice::Custom("Night".into()));
        state.save_status_playback(&path).expect("replace state");
        let saved = StatusPlayback::load_status_playback(Some(&path)).expect("read saved state");
        assert_eq!(saved, state);
        assert_eq!(
            saved
                .playing_playlist()
                .expect("active playlist")
                .slide_duration(),
            Duration::from_secs(604_860)
        );
    }
}
