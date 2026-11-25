//! Status wallpaper: decodes the background image once, at the display size.
//!
//! `img(path)` in GPUI keeps the image at its file size. A 3840 x 2160
//! wallpaper on a 1920 x 1080 display then uses four times the memory that
//! the display can show. This module scales the image down before GPUI
//! uploads it to the GPU.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use gpui::{RenderImage, Size};
use image::{DynamicImage, Frame, ImageDecoder as _, ImageReader};

use crate::status_style::StatusStyleName;

/// Lists the style wallpapers in `folder`: files named after a status style,
/// such as `cyrene-bow.png`. Other files stay unused. A missing folder gives
/// no wallpapers.
pub fn find_style_wallpapers(folder: &Path) -> HashMap<StatusStyleName, PathBuf> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) => {
            log::warn!("cannot read wallpaper folder {}: {error}", folder.display());
            return HashMap::new();
        }
    };
    let mut wallpapers = HashMap::new();
    let mut duplicates = HashSet::new();
    for path in entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
    {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        if !matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg")) {
            continue;
        }
        let Some(style) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(StatusStyleName::status_style_from_text)
        else {
            continue;
        };
        if duplicates.contains(&style) {
            continue;
        }
        if wallpapers.insert(style, path).is_some() {
            wallpapers.remove(&style);
            duplicates.insert(style);
            log::warn!("multiple wallpaper files for {}", style.style_name_text());
        }
    }
    wallpapers
}

/// Decodes the wallpaper and scales it down to the smallest size that still
/// covers `target`, in physical pixels. A smaller image keeps its size. This
/// call blocks; run it on a background thread.
pub fn load_status_wallpaper(path: &Path, target: Size<u32>) -> anyhow::Result<Arc<RenderImage>> {
    let mut decoder = ImageReader::open(path)
        .with_context(|| format!("cannot open wallpaper {}", path.display()))?
        .with_guessed_format()
        .context("cannot read the wallpaper format")?
        .into_decoder()
        .context("wallpaper format is not supported")?;
    let orientation = decoder
        .orientation()
        .context("cannot read the wallpaper orientation")?;
    let mut image = DynamicImage::from_decoder(decoder).context("cannot decode the wallpaper")?;
    image.apply_orientation(orientation);

    let (width, height) = wallpaper_cover_size(Size::new(image.width(), image.height()), target);
    if width < image.width() {
        image = image.thumbnail_exact(width, height);
    }
    let mut pixels = image.into_rgba8();
    // GPUI images are BGRA.
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(Arc::new(RenderImage::new(vec![Frame::new(pixels)])))
}

/// Smallest size with the aspect ratio of `image` that covers `target`. It
/// is never larger than `image`.
fn wallpaper_cover_size(image: Size<u32>, target: Size<u32>) -> (u32, u32) {
    let scale = (target.width as f64 / image.width.max(1) as f64)
        .max(target.height as f64 / image.height.max(1) as f64)
        .min(1.0);
    (
        ((image.width as f64 * scale).ceil() as u32).max(1),
        ((image.height as f64 * scale).ceil() as u32).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_covers_the_display_without_growing() {
        let image = Size::new(3840, 2160);
        assert_eq!(
            wallpaper_cover_size(image, Size::new(1920, 1080)),
            (1920, 1080)
        );
        // A taller display crops the sides.
        assert_eq!(
            wallpaper_cover_size(image, Size::new(1920, 1200)),
            (2134, 1200)
        );
        assert_eq!(
            wallpaper_cover_size(image, Size::new(5120, 2880)),
            (3840, 2160)
        );
    }
}
