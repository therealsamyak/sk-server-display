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
use rand::{Rng, seq::SliceRandom as _};
use serde::{Deserialize, Serialize};

use super::{
    status_import::{ImportedWallpaper, WallpaperId},
    status_style::{ALL_STATUS_STYLES, StatusStyle, StatusStyleName, status_style_by_name},
};

/// The default one-hour slideshow interval.
pub(super) const DEFAULT_SLIDESHOW_INTERVAL: NonZeroU32 = NonZeroU32::MIN.saturating_add(59);

/// The built-in slideshow that rotates all preset artwork.
pub(super) const ALL_SLIDESHOW_NAME: &str = "All";

/// List the artwork IDs in the current preset gallery in gallery order.
pub(super) fn preset_slideshow_styles(
    presets: Option<&HashMap<StatusStyleName, PathBuf>>,
) -> impl Iterator<Item = WallpaperId> + Clone {
    ALL_STATUS_STYLES
        .into_iter()
        .filter(move |style| {
            *style != StatusStyleName::Plain
                && presets.is_some_and(|paths| paths.contains_key(style))
        })
        .map(WallpaperId::Preset)
}

/// One ordered set of wallpapers and its time per slide.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StatusPlaylist {
    pub name: String,
    pub styles: Vec<WallpaperId>,
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
        current: Option<WallpaperId>,
        available: impl Fn(WallpaperId) -> bool,
    ) -> Option<WallpaperId> {
        let start = current
            .and_then(|style| self.styles.iter().position(|item| *item == style))
            .map_or(0, |index| index + 1);
        self.styles[start..]
            .iter()
            .chain(&self.styles[..start])
            .copied()
            .find(|style| available(*style))
    }

    /// Select the previous available slide before the current one, with wraparound.
    pub fn previous_slide(
        &self,
        current: Option<WallpaperId>,
        available: impl Fn(WallpaperId) -> bool,
    ) -> Option<WallpaperId> {
        let end = current
            .and_then(|style| self.styles.iter().position(|item| *item == style))
            .unwrap_or(self.styles.len());
        self.styles[..end]
            .iter()
            .rev()
            .chain(self.styles[end..].iter().rev())
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
        Self {
            favorites: StatusPlaylist {
                name: "Favorites".into(),
                styles: Vec::new(),
                interval_minutes: DEFAULT_SLIDESHOW_INTERVAL,
            },
            slideshows: Vec::new(),
        }
    }
}

impl PlaylistSettings {
    /// Find the saved slideshow named by this selection.
    pub fn selected_playlist(&self, choice: &PlaylistChoice) -> Option<&StatusPlaylist> {
        match choice {
            PlaylistChoice::All => None,
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

/// All preset artwork, Favorites, or a custom slideshow in the user's collection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlaylistChoice {
    All,
    Favorites,
    Custom(String),
}

/// Persistent mode: hold one wallpaper or advance an ordered slideshow.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", content = "selection", rename_all = "kebab-case")]
pub enum StatusPlaybackMode {
    Fixed(WallpaperId),
    Slideshow(PlaylistChoice),
}

/// Settings saved beside the hand-written server configuration.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StatusPlayback {
    pub settings: PlaylistSettings,
    pub imports: Vec<ImportedWallpaper>,
    pub playback: StatusPlaybackMode,
}

impl Default for StatusPlayback {
    fn default() -> Self {
        Self {
            settings: PlaylistSettings::default(),
            playback: StatusPlaybackMode::Fixed(WallpaperId::PLAIN),
            imports: Vec::new(),
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
        let mut state: Self = toml::from_str(&text).context("invalid slideshow state")?;
        let absolute = std::path::absolute(path).context("cannot resolve slideshow state path")?;
        let directory = absolute
            .parent()
            .context("slideshow state has no parent directory")?;
        for import in &mut state.imports {
            if import.path.is_relative() {
                import.path = directory.join(&import.path);
            }
        }
        state.validate_playback()?;
        Ok(state)
    }

    /// Save valid slideshow state in one atomic file replacement.
    pub fn save_status_playback(&self, path: &Path) -> anyhow::Result<()> {
        self.validate_playback()?;
        let directory = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut state = self.clone();
        let absolute = std::path::absolute(path).context("cannot resolve slideshow state path")?;
        let profile_directory = absolute
            .parent()
            .context("slideshow state has no parent directory")?;
        for import in &mut state.imports {
            if let Ok(relative) = import.path.strip_prefix(profile_directory) {
                let components = relative
                    .components()
                    .map(|part| {
                        part.as_os_str()
                            .to_str()
                            .context("wallpaper path is not valid UTF-8")
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                import.path = PathBuf::from(components.join("/"));
            }
        }
        let contents =
            toml::to_string_pretty(&state).context("cannot serialize slideshow state")?;
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

    fn validate_playback(&self) -> anyhow::Result<()> {
        self.settings.validate()?;
        let mut ids = HashSet::new();
        for import in &self.imports {
            import.validate_import()?;
            ensure!(ids.insert(import.id), "duplicate imported wallpaper ID");
        }
        Ok(())
    }

    /// Find an imported wallpaper by its stable selection.
    pub fn imported_wallpaper(&self, wallpaper: WallpaperId) -> Option<&ImportedWallpaper> {
        match wallpaper {
            WallpaperId::Preset(_) => None,
            WallpaperId::Imported(id) => self.imports.iter().find(|entry| entry.id == id),
        }
    }

    /// Return the current saved playlist when the display plays Favorites or a custom list.
    pub fn playing_saved_playlist(&self) -> Option<&StatusPlaylist> {
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
    pub presets: Option<Arc<HashMap<StatusStyleName, PathBuf>>>,
    pub current: WallpaperId,
    pub last_switch: Instant,
    failed: HashSet<WallpaperId>,
    all_playlist: StatusPlaylist,
}

impl StatusCarousel {
    /// Open the saved slideshow at its first available wallpaper.
    pub fn new_status_carousel(
        state: StatusPlayback,
        state_path: Option<PathBuf>,
        presets: Option<HashMap<StatusStyleName, PathBuf>>,
    ) -> Self {
        let mut carousel = Self {
            state,
            state_path,
            presets: presets.map(Arc::new),
            current: WallpaperId::PLAIN,
            last_switch: Instant::now(),
            failed: HashSet::new(),
            all_playlist: StatusPlaylist {
                name: ALL_SLIDESHOW_NAME.into(),
                styles: Vec::new(),
                interval_minutes: DEFAULT_SLIDESHOW_INTERVAL,
            },
        };
        carousel.select_initial_slide();
        carousel
    }

    /// Start the selected slideshow and persist the selection with its settings.
    pub fn play_status_playlist(
        &mut self,
        choice: PlaylistChoice,
        settings: PlaylistSettings,
    ) -> anyhow::Result<()> {
        let mut next = self.state.clone();
        next.settings = settings;
        next.playback = StatusPlaybackMode::Slideshow(choice);
        self.update_status_playback(next)?;
        self.select_initial_slide();
        Ok(())
    }

    fn select_initial_slide(&mut self) {
        if self.state.playback == StatusPlaybackMode::Slideshow(PlaylistChoice::All) {
            self.failed
                .retain(|id| matches!(id, WallpaperId::Imported(_)));
            self.refresh_all_playlist(&mut rand::rng());
        }
        self.select_first_available_slide();
    }

    fn select_first_available_slide(&mut self) {
        self.current = match &self.state.playback {
            StatusPlaybackMode::Fixed(style) if self.available(*style) => *style,
            StatusPlaybackMode::Fixed(_) => WallpaperId::PLAIN,
            StatusPlaybackMode::Slideshow(_) => self
                .next_available_slide(None)
                .unwrap_or(WallpaperId::PLAIN),
        };
        self.last_switch = Instant::now();
    }

    fn refresh_all_playlist(&mut self, rng: &mut impl Rng) {
        self.all_playlist.styles = preset_slideshow_styles(self.presets.as_deref()).collect();
        self.all_playlist.styles.shuffle(rng);
    }

    /// Return the active slideshow, including the current shuffled order for All.
    pub fn playing_playlist(&self) -> Option<&StatusPlaylist> {
        match &self.state.playback {
            StatusPlaybackMode::Slideshow(PlaylistChoice::All) => Some(&self.all_playlist),
            StatusPlaybackMode::Fixed(_)
            | StatusPlaybackMode::Slideshow(
                PlaylistChoice::Favorites | PlaylistChoice::Custom(_),
            ) => self.state.playing_saved_playlist(),
        }
    }

    fn available(&self, style: WallpaperId) -> bool {
        style == WallpaperId::PLAIN || self.wallpaper_path(style).is_some()
    }

    /// Update the preset gallery while preserving the active slide when available.
    pub fn replace_wallpaper_presets(
        &mut self,
        presets: Option<HashMap<StatusStyleName, PathBuf>>,
    ) -> bool {
        let changed = self.presets.as_deref() != presets.as_ref();
        let playing_all = self.state.playback == StatusPlaybackMode::Slideshow(PlaylistChoice::All);
        let retry_all = playing_all
            && self
                .failed
                .iter()
                .any(|id| matches!(id, WallpaperId::Preset(_)));
        if !changed && !retry_all {
            return false;
        }
        self.presets = presets.map(Arc::new);
        self.failed
            .retain(|id| matches!(id, WallpaperId::Imported(_)));
        if playing_all && changed {
            self.refresh_all_playlist(&mut rand::rng());
        }
        if self.current == WallpaperId::PLAIN || !self.available(self.current) {
            self.select_first_available_slide();
        }
        true
    }

    /// The real wallpaper path for a style, if decoding has succeeded or is untried.
    pub fn wallpaper_path(&self, style: WallpaperId) -> Option<&Path> {
        if self.failed.contains(&style) {
            return None;
        }
        match style {
            WallpaperId::Preset(preset) => {
                self.presets.as_ref()?.get(&preset).map(PathBuf::as_path)
            }
            WallpaperId::Imported(_) => self
                .state
                .imported_wallpaper(style)
                .map(|entry| entry.path.as_path()),
        }
    }

    /// Resolve the layout and palette belonging to a wallpaper.
    pub fn wallpaper_style(&self, wallpaper: WallpaperId) -> StatusStyle {
        match wallpaper {
            WallpaperId::Preset(preset) => *status_style_by_name(preset),
            WallpaperId::Imported(_) => self.state.imported_wallpaper(wallpaper).map_or(
                *status_style_by_name(StatusStyleName::Plain),
                ImportedWallpaper::wallpaper_style,
            ),
        }
    }

    /// Save an import or edit and show it as a fixed wallpaper.
    pub fn apply_imported_wallpaper(&mut self, import: ImportedWallpaper) -> anyhow::Result<()> {
        let id = WallpaperId::Imported(import.id);
        let mut next = self.state.clone();
        if let Some(existing) = next
            .imports
            .iter_mut()
            .find(|existing| existing.id == import.id)
        {
            *existing = import;
        } else {
            next.imports.push(import);
        }
        next.playback = StatusPlaybackMode::Fixed(id);
        self.update_status_playback(next)?;
        self.failed.remove(&id);
        self.current = id;
        Ok(())
    }

    /// True when saved playback requests art and none of it can be displayed.
    pub fn playback_art_unavailable(&self) -> bool {
        if self.current != WallpaperId::PLAIN {
            return false;
        }
        match &self.state.playback {
            StatusPlaybackMode::Fixed(id) => *id != WallpaperId::PLAIN,
            StatusPlaybackMode::Slideshow(choice) => {
                self.playing_playlist().is_some_and(|playlist| {
                    (matches!(choice, PlaylistChoice::All) || !playlist.styles.is_empty())
                        && self.next_available_slide(None).is_none()
                })
            }
        }
    }

    /// Find the next slideshow entry with an available wallpaper.
    pub fn next_available_slide(&self, current: Option<WallpaperId>) -> Option<WallpaperId> {
        self.playing_playlist()?
            .next_slide(current, |style| self.available(style))
    }

    /// The slideshow wallpaper that a step forward or back shows, with wraparound.
    pub fn adjacent_slide(&self, forward: bool) -> Option<WallpaperId> {
        let list = self.playing_playlist()?;
        let current = Some(self.current);
        let available = |style| self.available(style);
        if forward {
            list.next_slide(current, available)
        } else {
            list.previous_slide(current, available)
        }
    }

    /// Show the next or previous slideshow wallpaper and restart its slide timer.
    pub fn step_slideshow(&mut self, forward: bool) {
        if self.playing_playlist().is_none() {
            return;
        }
        if let Some(step) = self.adjacent_slide(forward) {
            self.current = step;
        }
        self.last_switch = Instant::now();
    }

    /// Exclude bad art from later rotation and return the next usable slide.
    pub fn skip_failed_wallpaper(&mut self, style: WallpaperId) {
        self.failed.insert(style);
        if style == self.current {
            self.current = self
                .next_available_slide(Some(style))
                .unwrap_or(WallpaperId::PLAIN);
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
    use rand::{SeedableRng as _, rngs::StdRng};

    use super::*;
    use StatusStyleName::*;
    use WallpaperId::Preset;

    #[test]
    fn all_slideshow_shuffles_and_loops_the_preset_gallery() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.toml");
        let gallery = [CyreneBow, FurinaDeep, YolandaKokomi, Plain];
        let presets = gallery
            .into_iter()
            .map(|style| (style, directory.path().join(style.style_name_text())))
            .collect();
        let state = StatusPlayback::default();
        let mut carousel =
            StatusCarousel::new_status_carousel(state.clone(), Some(path.clone()), Some(presets));
        carousel
            .play_status_playlist(PlaylistChoice::All, state.settings.clone())
            .unwrap();
        let saved = StatusPlayback::load_status_playback(Some(&path)).unwrap();
        assert_eq!(
            saved.playback,
            StatusPlaybackMode::Slideshow(PlaylistChoice::All)
        );
        assert_eq!(saved.settings, state.settings);

        carousel.refresh_all_playlist(&mut StdRng::seed_from_u64(42));
        carousel.select_first_available_slide();
        let playlist = carousel.playing_playlist().unwrap();
        let ordered = &playlist.styles;
        let expected = [Preset(YolandaKokomi), Preset(FurinaDeep), Preset(CyreneBow)];
        assert_eq!(
            ordered.iter().copied().collect::<HashSet<_>>(),
            HashSet::from(expected)
        );
        assert_ne!(ordered, &expected);
        assert_eq!(ordered.len(), expected.len());
        assert_eq!(
            playlist.slide_duration(),
            state.settings.favorites.slide_duration()
        );
        let first = carousel.current;
        let mut cycle = Vec::new();
        for _ in &expected {
            cycle.push(carousel.current);
            carousel.step_slideshow(true);
        }
        assert_eq!(carousel.current, first);
        for expected in cycle.into_iter().rev() {
            carousel.step_slideshow(false);
            assert_eq!(carousel.current, expected);
        }
    }

    #[test]
    fn all_slideshow_tracks_preset_source_changes_after_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.toml");
        let state = StatusPlayback {
            playback: StatusPlaybackMode::Slideshow(PlaylistChoice::All),
            ..StatusPlayback::default()
        };
        state.save_status_playback(&path).unwrap();
        let mut restarted = StatusCarousel::new_status_carousel(
            StatusPlayback::load_status_playback(Some(&path)).unwrap(),
            Some(path),
            Some(HashMap::from([(
                CyreneBow,
                directory.path().join("cyrene.png"),
            )])),
        );
        assert_eq!(restarted.current, Preset(CyreneBow));

        assert!(restarted.replace_wallpaper_presets(Some(HashMap::from([
            (CyreneBow, directory.path().join("new/cyrene.png")),
            (FurinaDeep, directory.path().join("new/furina.png")),
        ]))));
        assert_eq!(restarted.current, Preset(CyreneBow));
        assert_eq!(restarted.adjacent_slide(true), Some(Preset(FurinaDeep)));

        assert!(restarted.replace_wallpaper_presets(None));
        assert_eq!(restarted.current, WallpaperId::PLAIN);
        assert_eq!(restarted.adjacent_slide(true), None);
        assert!(restarted.playback_art_unavailable());
    }

    #[test]
    fn all_slideshow_skips_lost_art_and_retries_when_refreshed() {
        let directory = tempfile::tempdir().unwrap();
        let presets = HashMap::from([
            (CyreneBow, directory.path().join("cyrene.png")),
            (FurinaDeep, directory.path().join("missing.png")),
        ]);
        let state = StatusPlayback {
            playback: StatusPlaybackMode::Slideshow(PlaylistChoice::All),
            ..StatusPlayback::default()
        };
        let mut carousel = StatusCarousel::new_status_carousel(state, None, Some(presets.clone()));

        carousel.skip_failed_wallpaper(Preset(FurinaDeep));
        assert_eq!(carousel.current, Preset(CyreneBow));
        assert_eq!(carousel.adjacent_slide(false), Some(Preset(CyreneBow)));

        assert!(carousel.replace_wallpaper_presets(Some(presets)));
        assert_eq!(carousel.current, Preset(CyreneBow));
        assert_eq!(carousel.adjacent_slide(false), Some(Preset(FurinaDeep)));
    }

    #[test]
    fn rotation_skips_missing_art_and_wraps_in_order() {
        let list = StatusPlaylist {
            name: "Test".into(),
            styles: vec![Preset(CyreneBow), Preset(FurinaDeep), Preset(YolandaKokomi)],
            interval_minutes: NonZeroU32::MIN,
        };
        let available = |style| style != Preset(FurinaDeep);
        assert_eq!(
            list.next_slide(Some(Preset(CyreneBow)), available),
            Some(Preset(YolandaKokomi))
        );
        assert_eq!(
            list.next_slide(Some(Preset(YolandaKokomi)), available),
            Some(Preset(CyreneBow))
        );
        assert_eq!(list.next_slide(None, |_| false), None);
        assert_eq!(
            list.previous_slide(Some(Preset(CyreneBow)), available),
            Some(Preset(YolandaKokomi))
        );
        assert_eq!(
            list.previous_slide(Some(Preset(YolandaKokomi)), available),
            Some(Preset(CyreneBow))
        );
    }

    #[test]
    fn round_trip_replaces_saved_selection_and_preserves_long_minute_interval() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("state.toml");
        let mut state = StatusPlayback::load_status_playback(Some(&path)).expect("default state");
        state.save_status_playback(&path).expect("initial save");
        state.settings.slideshows.push(StatusPlaylist {
            name: "Night".into(),
            styles: vec![Preset(CyreneBow), Preset(YolandaKokomi)],
            interval_minutes: NonZeroU32::new(10_081).expect("nonzero"),
        });
        state.playback = StatusPlaybackMode::Slideshow(PlaylistChoice::Custom("Night".into()));
        state.save_status_playback(&path).expect("replace state");
        let saved = StatusPlayback::load_status_playback(Some(&path)).expect("read saved state");
        assert_eq!(saved, state);
        assert_eq!(
            saved
                .playing_saved_playlist()
                .expect("active playlist")
                .slide_duration(),
            Duration::from_secs(604_860)
        );
    }

    #[test]
    fn imported_layout_stays_with_its_id_after_edit_and_restart() {
        use crate::wallpaper::status_import::{PanelAppearance, PanelPlacement};
        use crate::wallpaper::status_style::StyleArrangement;

        let directory = tempfile::tempdir().unwrap();
        let state = directory.path().join("state.toml");
        let mut carousel = StatusCarousel::new_status_carousel(
            StatusPlayback::default(),
            Some(state.clone()),
            None,
        );
        let id = NonZeroU32::MIN;
        let imported = WallpaperId::Imported(id);
        let path = directory.path().join("photo.jpg");
        let mut wallpaper = ImportedWallpaper {
            id,
            name: "Test photo".into(),
            path: path.clone(),
            placement: PanelPlacement::Left,
            appearance: PanelAppearance::Glass,
        };
        carousel
            .apply_imported_wallpaper(wallpaper.clone())
            .unwrap();
        let mut playback = carousel.state.clone();
        playback.settings.favorites.styles.push(imported);
        carousel.update_status_playback(playback).unwrap();

        wallpaper.placement = PanelPlacement::Right;
        wallpaper.appearance = PanelAppearance::Opaque;
        carousel.apply_imported_wallpaper(wallpaper).unwrap();
        let persisted = StatusPlayback::load_status_playback(Some(&state)).unwrap();
        let restarted = StatusCarousel::new_status_carousel(persisted, Some(state), None);

        assert_eq!(restarted.state.imports.len(), 1);
        assert_eq!(restarted.state.settings.favorites.styles, [imported]);
        assert_eq!(restarted.current, imported);
        assert_eq!(restarted.wallpaper_path(imported), Some(path.as_path()));
        let style = restarted.wallpaper_style(imported);
        assert!(
            matches!(style.arrangement, StyleArrangement::Sides { left, right } if left.groups.is_empty() && left.clock.is_some() && right.groups.len() == 2 && right.clock.is_none())
        );
        assert!(style.palette.panel.1 >= 0.99);
    }

    #[test]
    fn failed_import_yields_solid_background_and_preserves_saved_selection() {
        use crate::wallpaper::status_import::{PanelAppearance, PanelPlacement};

        let directory = tempfile::tempdir().unwrap();
        let id = NonZeroU32::MIN;
        let imported = WallpaperId::Imported(id);
        let mut carousel = StatusCarousel::new_status_carousel(
            StatusPlayback::default(),
            Some(directory.path().join("state.toml")),
            None,
        );
        carousel
            .apply_imported_wallpaper(ImportedWallpaper {
                id,
                name: "Missing photo".into(),
                path: directory.path().join("missing.jpg"),
                placement: PanelPlacement::Split,
                appearance: PanelAppearance::Glass,
            })
            .unwrap();

        carousel.skip_failed_wallpaper(imported);

        assert_eq!(carousel.current, WallpaperId::PLAIN);
        assert!(carousel.playback_art_unavailable());
        assert_eq!(carousel.state.playback, StatusPlaybackMode::Fixed(imported));
    }
}
