//! Copy a display profile and its artwork into a portable directory.

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, ensure};
use image::{ImageFormat, ImageReader};

use crate::{
    metrics::{StatusConfig, load_status_config, status_state_path},
    wallpaper::{StatusPlayback, find_style_wallpapers},
};

/// Filename of the configuration in a portable profile.
pub const PROFILE_CONFIG_NAME: &str = "config.toml";

/// Copy the active profile into a new directory.
pub fn export_profile(source: Option<&Path>, destination: &Path) -> anyhow::Result<()> {
    copy_profile(source, &destination.join(PROFILE_CONFIG_NAME), None)
}

/// Import a portable directory as a new profile, leaving existing profiles intact.
pub fn import_profile(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let source = source
        .canonicalize()
        .context("cannot find portable profile")?;
    ensure!(source.is_dir(), "select a portable profile directory");
    let config = source.join(PROFILE_CONFIG_NAME);
    let state = status_state_path(Some(&config)).context("profile has no state path")?;
    for file in [&config, &state] {
        portable_asset(file, Some(&source))?;
        ensure!(
            file.is_file(),
            "portable profile is missing {}",
            file.display()
        );
    }
    copy_profile(Some(&config), destination, Some(&source))
}

fn copy_profile(
    source: Option<&Path>,
    config_path: &Path,
    portable_root: Option<&Path>,
) -> anyhow::Result<()> {
    let config_path =
        std::path::absolute(config_path).context("cannot resolve profile destination")?;
    let destination = config_path.parent().context("profile needs a directory")?;
    ensure!(
        !destination.exists(),
        "choose a new profile directory: {}",
        destination.display()
    );
    ensure!(
        status_state_path(Some(&config_path)).as_deref() != Some(config_path.as_path()),
        "the configuration filename cannot be the same as the state file"
    );

    let source = source
        .map(std::path::absolute)
        .transpose()
        .context("cannot resolve source profile")?;
    let mut config = load_status_config(source.as_deref())?;
    let mut state =
        StatusPlayback::load_status_playback(status_state_path(source.as_deref()).as_deref())?;
    let parent = destination
        .parent()
        .context("profile cannot replace the filesystem root")?;
    fs::create_dir_all(parent).context("cannot create profile parent folder")?;
    let staging = tempfile::Builder::new()
        .prefix(".server-status-profile-")
        .tempdir_in(parent)
        .context("cannot stage portable profile")?;
    let staging_path = staging.path();

    copy_imports(&mut state, staging_path, portable_root)?;
    copy_presets(&mut config, staging_path, portable_root)?;

    let filename = config_path
        .file_name()
        .context("profile needs a configuration filename")?;
    let staged_config = staging_path.join(filename);
    let state_path =
        status_state_path(Some(&staged_config)).context("profile needs a state path")?;
    state.save_status_playback(&state_path)?;
    let mut config_file =
        fs::File::create(&staged_config).context("cannot write profile configuration")?;
    config_file.write_all(toml::to_string_pretty(&config)?.as_bytes())?;
    config_file.sync_all()?;
    drop(config_file);

    publish_profile(staging_path, destination, &staged_config)
}

fn copy_imports(
    state: &mut StatusPlayback,
    staging: &Path,
    portable_root: Option<&Path>,
) -> anyhow::Result<()> {
    if state.imports.is_empty() {
        return Ok(());
    }
    let assets = staging.join("images");
    fs::create_dir(&assets).context("cannot create profile images directory")?;
    for image in &mut state.imports {
        let context = || format!("cannot copy imported wallpaper {}", image.name);
        let source = portable_asset(&image.path, portable_root).with_context(context)?;
        ensure!(
            source.is_file(),
            "wallpaper is not a regular file: {}",
            image.name
        );
        let format = ImageReader::open(&source)
            .with_context(context)?
            .with_guessed_format()
            .with_context(context)?
            .format();
        let extension = match format {
            Some(ImageFormat::Png) => "png",
            Some(ImageFormat::Jpeg) => "jpg",
            _ => anyhow::bail!("wallpaper is not PNG or JPEG: {}", image.name),
        };
        let destination = assets.join(format!("wallpaper-{}.{}", image.id, extension));
        fs::copy(source, &destination).with_context(context)?;
        image.path = destination;
    }
    Ok(())
}

fn copy_presets(
    config: &mut StatusConfig,
    staging: &Path,
    portable_root: Option<&Path>,
) -> anyhow::Result<()> {
    let Some(folder) = &config.preset_wallpapers else {
        return Ok(());
    };
    let source = portable_asset(folder, portable_root)?;
    let preset_folder = PathBuf::from("presets");
    let destination = staging.join(&preset_folder);
    fs::create_dir(&destination).context("cannot create profile presets directory")?;
    for image in find_style_wallpapers(&source).into_values() {
        let name = image
            .file_name()
            .context("preset is missing its filename")?;
        let asset = portable_asset(&image, portable_root)?;
        fs::copy(asset, destination.join(name)).context("cannot copy preset wallpaper")?;
    }
    config.preset_wallpapers = Some(preset_folder);
    Ok(())
}

fn portable_asset(path: &Path, portable_root: Option<&Path>) -> anyhow::Result<PathBuf> {
    let Some(root) = portable_root else {
        return Ok(path.to_path_buf());
    };
    let path = path
        .canonicalize()
        .context("cannot read asset in portable profile")?;
    ensure!(
        path.starts_with(root),
        "portable profile references a file outside its folder"
    );
    Ok(path)
}

fn publish_profile(staging: &Path, destination: &Path, config: &Path) -> anyhow::Result<()> {
    fs::create_dir(destination)
        .with_context(|| format!("cannot create new profile at {}", destination.display()))?;
    let publish = || -> anyhow::Result<()> {
        for entry in fs::read_dir(staging)? {
            let entry = entry?;
            if entry.path() != config {
                fs::rename(entry.path(), destination.join(entry.file_name()))?;
            }
        }
        let filename = config
            .file_name()
            .context("profile needs a configuration filename")?;
        fs::rename(config, destination.join(filename))?;
        Ok(())
    };
    if let Err(error) = publish() {
        if let Err(cleanup) = fs::remove_dir_all(destination) {
            log::warn!("could not remove an incomplete profile: {cleanup}");
        }
        return Err(error).context("cannot save portable profile");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallpaper::{ImportedWallpaper, StatusPlaybackMode, WallpaperId};
    use std::num::NonZeroU32;

    #[test]
    fn portable_profile_keeps_wallpapers_and_favorites_after_export_and_import() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        let config = source.join(PROFILE_CONFIG_NAME);
        fs::write(
            &config,
            "[[host]]\nname = 'Example host'\nsource = 'glances'\nurl = 'http://127.0.0.1:61208'\n",
        )
        .unwrap();
        let art = root.join("photo.png");
        image::RgbaImage::new(2, 2).save(&art).unwrap();
        let id = NonZeroU32::MIN;
        let selection = WallpaperId::Imported(id);
        let mut original = StatusPlayback::default();
        original.imports.push(ImportedWallpaper {
            id,
            name: "Photo".into(),
            path: art.clone(),
            placement: Default::default(),
            appearance: Default::default(),
        });
        original.settings.favorites.styles.push(selection);
        original.playback = StatusPlaybackMode::Fixed(selection);
        original
            .save_status_playback(&status_state_path(Some(&config)).unwrap())
            .unwrap();

        let portable = root.join("portable");
        export_profile(Some(&config), &portable).unwrap();
        let restored = root.join("restored").join(PROFILE_CONFIG_NAME);
        let exported_state = status_state_path(Some(&portable.join(PROFILE_CONFIG_NAME))).unwrap();
        assert!(
            fs::read_to_string(exported_state)
                .unwrap()
                .contains("images/wallpaper-1.png")
        );
        import_profile(&portable, &restored).unwrap();
        fs::remove_file(art).unwrap();
        fs::remove_dir_all(portable).unwrap();

        let restored_config = load_status_config(Some(&restored)).unwrap();
        assert_eq!(restored_config, load_status_config(Some(&config)).unwrap());
        let state_path = status_state_path(Some(&restored)).unwrap();
        let mut state = StatusPlayback::load_status_playback(Some(&state_path)).unwrap();
        let restored_art = state.imports[0].path.clone();
        assert!(restored_art.starts_with(root.join("restored")));
        assert!(image::open(restored_art).is_ok());
        state.imports[0].path = original.imports[0].path.clone();
        assert_eq!(state, original);
    }

    #[test]
    fn import_rejects_art_outside_the_selected_profile() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let portable = root.join("portable");
        fs::create_dir(&portable).unwrap();
        let config = portable.join(PROFILE_CONFIG_NAME);
        fs::write(&config, "").unwrap();
        let art = root.join("outside.png");
        image::RgbaImage::new(1, 1).save(&art).unwrap();
        let mut state = StatusPlayback::default();
        state.imports.push(ImportedWallpaper {
            id: NonZeroU32::MIN,
            name: "Outside".into(),
            path: art,
            placement: Default::default(),
            appearance: Default::default(),
        });
        state
            .save_status_playback(&status_state_path(Some(&config)).unwrap())
            .unwrap();

        let destination = root.join("restored");
        let result = import_profile(&portable, &destination.join(PROFILE_CONFIG_NAME));

        assert!(result.is_err());
        assert!(!destination.exists());
    }

    #[test]
    fn export_does_not_replace_an_existing_profile() {
        let temporary = tempfile::tempdir().unwrap();
        let existing = temporary.path().join(PROFILE_CONFIG_NAME);
        fs::write(&existing, "# keep me\n").unwrap();

        let result = export_profile(Some(&existing), temporary.path());

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(existing).unwrap(), "# keep me\n");
    }
}
