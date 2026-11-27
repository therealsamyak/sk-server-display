//! Wallpaper loading and preview for the status window.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use gpui::{
    AppContext as _, Context, Entity, IntoElement, RenderImage, Size, Subscription, Task, Window,
    div, prelude::*,
};

use super::{StatusView, status_view_layout::SCREEN_PADDING};
use crate::dashboard::status_theme::Scale;
use crate::wallpaper::{
    ImportedWallpaper, PlaylistChoice, StatusPicker, StatusPickerEvent, StatusPlaybackMode,
    StatusStyle, WallpaperId, load_status_wallpaper, plain_status_style,
};

pub(super) struct StatusWallpaper {
    pub(super) shown: WallpaperId,
    pub(super) style: StatusStyle,
    pub(super) image: Option<Arc<RenderImage>>,
    requested: Option<(WallpaperId, Size<u32>)>,
    ready: Option<WallpaperFrame>,
    task: Option<Task<()>>,
    slideshow: Option<SlideshowImages>,
}

struct SlideshowImages {
    target: Size<u32>,
    paths: HashMap<WallpaperId, PathBuf>,
    images: HashMap<WallpaperId, Arc<RenderImage>>,
    _loader: Task<()>,
}

pub(super) struct PickerSession {
    pub(super) view: Entity<StatusPicker>,
    original_style: WallpaperId,
    _events: Subscription,
}

struct WallpaperFrame {
    id: WallpaperId,
    style: StatusStyle,
    image: Option<Arc<RenderImage>>,
}

pub(super) struct WallpaperPreview {
    id: WallpaperId,
    draft: Option<ImportedWallpaper>,
    pub(super) title: String,
}

impl Default for StatusWallpaper {
    fn default() -> Self {
        Self {
            shown: WallpaperId::PLAIN,
            style: plain_status_style(),
            image: None,
            requested: None,
            ready: None,
            task: None,
            slideshow: None,
        }
    }
}

impl StatusWallpaper {
    /// A decoded slideshow image for the current window size.
    pub(super) fn warm_image(&self, id: WallpaperId, window: &Window) -> Option<Arc<RenderImage>> {
        let slideshow = self.slideshow.as_ref()?;
        if slideshow.target != wallpaper_target(window) {
            return None;
        }
        slideshow.images.get(&id).cloned()
    }
}

impl StatusView {
    /// Request fresh wallpaper images after the profile's presets change.
    pub(super) fn reload_preset_wallpapers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker.is_some() {
            self.close_picker(window, cx);
        }
        self.wallpaper.requested = None;
        self.wallpaper.ready = None;
        self.wallpaper.task = None;
        self.warm_slideshow(window, cx);
    }

    /// Tap area in the left or right window margin. A tap shows the previous
    /// or next slideshow wallpaper and restarts the slide timer.
    pub(super) fn slideshow_edge(
        &self,
        s: Scale,
        forward: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(if forward {
                "slideshow-next-edge"
            } else {
                "slideshow-previous-edge"
            })
            .absolute()
            .top_0()
            .bottom_0()
            .when(forward, |edge| edge.right_0())
            .when(!forward, |edge| edge.left_0())
            .w(s(SCREEN_PADDING))
            .on_click(cx.listener(move |view, _, _, cx| {
                if view.swipe_consumed || view.swipe_in_motion() {
                    return;
                }
                view.carousel
                    .update(cx, |carousel, _| carousel.step_slideshow(forward));
                cx.notify();
            }))
    }

    /// Decode the active playlist in the background, starting with both neighbors.
    pub(super) fn warm_slideshow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker.is_some() || self.wallpaper.task.is_some() {
            return;
        }
        let carousel = self.carousel.read(cx);
        let target = wallpaper_target(window);
        let entries = carousel
            .state
            .playing_playlist()
            .into_iter()
            .flat_map(|playlist| &playlist.styles)
            .filter_map(|&id| Some((id, carousel.wallpaper_path(id)?)));
        if self.wallpaper.slideshow.as_ref().is_some_and(|cache| {
            cache.target == target
                && cache.paths.len() == entries.clone().count()
                && entries
                    .clone()
                    .all(|(id, path)| cache.paths.get(&id).map(PathBuf::as_path) == Some(path))
        }) {
            return;
        }
        let paths: HashMap<_, _> = entries.map(|(id, path)| (id, path.to_owned())).collect();
        let mut images = HashMap::new();
        if let Some(cache) = self.wallpaper.slideshow.take() {
            for (id, image) in cache.images {
                if cache.target == target && paths.get(&id) == cache.paths.get(&id) {
                    images.insert(id, image);
                } else if self
                    .wallpaper
                    .image
                    .as_ref()
                    .is_none_or(|shown| !Arc::ptr_eq(shown, &image))
                    && let Err(error) = window.drop_image(image)
                {
                    log::debug!("cannot release cached wallpaper: {error:#}");
                }
            }
        }
        if paths.is_empty() {
            return;
        }
        if self.wallpaper.ready.is_none()
            && let Some((id, size)) = self.wallpaper.requested
            && size == target
            && id == self.wallpaper.shown
            && paths.contains_key(&id)
            && let Some(image) = &self.wallpaper.image
        {
            images.entry(id).or_insert_with(|| image.clone());
        }

        let mut queue: Vec<_> = paths
            .iter()
            .filter(|(id, _)| !images.contains_key(id))
            .map(|(&id, path)| (id, path.clone()))
            .collect();
        let next = carousel.adjacent_slide(true);
        let previous = carousel.adjacent_slide(false);
        queue.sort_by_key(|(id, _)| {
            if Some(*id) == next {
                0
            } else if Some(*id) == previous {
                1
            } else {
                2
            }
        });
        let loader = cx.spawn_in(window, async move |view, window| {
            for (id, path) in queue {
                let result = window
                    .background_spawn(async move { load_status_wallpaper(&path, target) })
                    .await;
                if view
                    .update_in(window, |view, _, cx| {
                        match result {
                            Ok(image) => {
                                if let Some(cache) = view.wallpaper.slideshow.as_mut() {
                                    cache.images.entry(id).or_insert(image);
                                }
                            }
                            Err(error) => {
                                log::warn!(
                                    "cannot preload wallpaper for {}: {error:#}",
                                    id.wallpaper_key()
                                );
                                view.carousel
                                    .update(cx, |carousel, _| carousel.skip_failed_wallpaper(id));
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        self.wallpaper.slideshow = Some(SlideshowImages {
            target,
            paths,
            images,
            _loader: loader,
        });
    }

    // Commit the image and the layout together. A ready image waits for the
    // current tap to end before it can move the cards.
    pub(super) fn update_wallpaper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let carousel = self.carousel.read(cx);
        let id = self
            .preview
            .as_ref()
            .map(|preview| preview.id)
            .or_else(|| self.picker.as_ref().map(|picker| picker.original_style))
            .unwrap_or(carousel.current);
        let target = wallpaper_target(window);
        if self.wallpaper.requested != Some((id, target)) {
            self.wallpaper.requested = Some((id, target));
            self.wallpaper.task = None;
            self.wallpaper.ready = None;
            let draft = self
                .preview
                .as_ref()
                .and_then(|preview| preview.draft.as_ref());
            let style = draft.map_or_else(
                || carousel.wallpaper_style(id),
                ImportedWallpaper::wallpaper_style,
            );
            let path = draft
                .map(|draft| draft.path.as_path())
                .or_else(|| carousel.wallpaper_path(id));
            let cached = self.wallpaper.slideshow.as_ref().and_then(|cache| {
                (draft.is_none()
                    && cache.target == target
                    && path == cache.paths.get(&id).map(PathBuf::as_path))
                .then(|| cache.images.get(&id).cloned())
                .flatten()
            });
            if let Some(image) = cached {
                self.wallpaper.ready = Some(WallpaperFrame {
                    id,
                    style,
                    image: Some(image),
                });
            } else if let Some(path) = path.map(ToOwned::to_owned) {
                let load = cx.background_spawn(async move { load_status_wallpaper(&path, target) });
                self.wallpaper.task = Some(cx.spawn_in(window, async move |this, cx| {
                    let result = load.await;
                    if let Err(error) = this.update_in(cx, |view, _, cx| {
                        view.wallpaper.task = None;
                        match result {
                            Ok(image) => {
                                view.wallpaper.ready = Some(WallpaperFrame {
                                    id,
                                    style,
                                    image: Some(image),
                                })
                            }
                            Err(error) => {
                                log::warn!(
                                    "cannot load wallpaper for {}: {error:#}",
                                    id.wallpaper_key()
                                );
                                if view.preview.is_some() {
                                    view.preview_error =
                                        Some("Could not read the image. Check the file path and choose a valid PNG or JPEG.".into());
                                } else {
                                    view.carousel.update(cx, |carousel, _| {
                                        carousel.skip_failed_wallpaper(id);
                                    });
                                    view.wallpaper.requested = None;
                                }
                            }
                        }
                        cx.notify();
                    }) {
                        log::debug!("status window closed before wallpaper loaded: {error:#}");
                    }
                }));
            } else {
                self.wallpaper.ready = Some(WallpaperFrame {
                    id,
                    style,
                    image: None,
                });
            }
        }
        if !self.pointer_pressed
            && !self.swipe_in_motion()
            && let Some(frame) = self.wallpaper.ready.take()
        {
            if let Some(old) = std::mem::replace(&mut self.wallpaper.image, frame.image)
                && self
                    .wallpaper
                    .image
                    .as_ref()
                    .is_none_or(|shown| !Arc::ptr_eq(shown, &old))
                && self.wallpaper.slideshow.as_ref().is_none_or(|cache| {
                    !cache
                        .images
                        .get(&self.wallpaper.shown)
                        .is_some_and(|cached| Arc::ptr_eq(cached, &old))
                })
                && let Err(error) = window.drop_image(old)
            {
                log::debug!("cannot release previous wallpaper: {error:#}");
            }
            self.wallpaper.shown = frame.id;
            self.wallpaper.style = frame.style;
            for panel in &self.panels {
                panel.update(cx, |panel, cx| panel.set_panel_style(frame.style, cx));
            }
            if let Some(details) = &self.details {
                details
                    .panel
                    .update(cx, |panel, cx| panel.set_panel_style(frame.style, cx));
            }
            if let Some(btop) = &self.btop {
                btop.view
                    .update(cx, |btop, cx| btop.set_btop_style(frame.style, cx));
            }
        }
    }

    pub(super) fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.details = None;
        self.incoming_panels = None;
        let carousel = self.carousel.read(cx);
        let active = self.wallpaper.shown;
        let presets = carousel.presets.clone();
        let imports = carousel.state.imports.clone();
        let settings = carousel.state.settings.clone();
        let picker = cx.new(|cx| {
            StatusPicker::new_status_picker(active, presets, imports, settings, window, cx)
        });
        let subscription = cx.subscribe_in(&picker, window, Self::handle_picker_event);
        self.picker = Some(PickerSession {
            view: picker,
            original_style: active,
            _events: subscription,
        });
        cx.notify();
    }

    fn handle_picker_event(
        &mut self,
        picker: &Entity<StatusPicker>,
        event: &StatusPickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = match event {
            StatusPickerEvent::Preview { wallpaper, draft } => {
                let title = draft
                    .as_ref()
                    .or_else(|| self.carousel.read(cx).state.imported_wallpaper(*wallpaper))
                    .map_or_else(|| wallpaper.wallpaper_title(), |entry| entry.name.clone());
                self.preview = Some(WallpaperPreview {
                    id: *wallpaper,
                    draft: draft.clone(),
                    title,
                });
                self.wallpaper.requested = None;
                self.preview_error = None;
                self.focus_handle.focus(window, cx);
                cx.notify();
                return;
            }
            StatusPickerEvent::Cancel => {
                self.close_picker(window, cx);
                return;
            }
            StatusPickerEvent::Save { settings, rename } => {
                self.carousel.update(cx, |carousel, _| {
                    let mut next = carousel.state.clone();
                    next.settings = settings.clone();
                    if let StatusPlaybackMode::Slideshow(PlaylistChoice::Custom(active)) =
                        &mut next.playback
                    {
                        if let Some((old, new)) = rename
                            && active == old
                        {
                            *active = new.clone();
                        }
                        if next.playing_playlist().is_none() {
                            next.playback =
                                StatusPlaybackMode::Slideshow(PlaylistChoice::Favorites);
                        }
                    }
                    carousel.update_status_playback(next)
                })
            }
            StatusPickerEvent::Play { choice, settings } => {
                let result = self.carousel.update(cx, |carousel, _| {
                    let mut next = carousel.state.clone();
                    next.settings = settings.clone();
                    next.playback = StatusPlaybackMode::Slideshow(choice.clone());
                    carousel.update_status_playback(next)?;
                    carousel.select_initial_slide();
                    Ok(())
                });
                if result.is_ok() {
                    self.close_picker(window, cx);
                    return;
                }
                result
            }
        };
        picker.update(cx, |picker, cx| {
            picker.set_save_error(result.err().map(|error| format!("{error:#}")), cx)
        });
        cx.notify();
    }

    pub(super) fn close_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview = None;
        self.preview_error = None;
        self.picker = None;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(super) fn cancel_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview = None;
        self.wallpaper.requested = None;
        self.preview_error = None;
        if let Some(picker) = &self.picker {
            picker
                .view
                .update(cx, |picker, cx| picker.focus_status_picker(window, cx));
        }
        cx.notify();
    }

    pub(super) fn apply_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(preview) = &self.preview else { return };
        if !self.preview_ready(preview) {
            return;
        }
        let id = preview.id;
        let draft = preview.draft.clone();
        match self.carousel.update(cx, |carousel, _| {
            if let Some(draft) = draft {
                return carousel.apply_imported_wallpaper(draft);
            }
            let mut next = carousel.state.clone();
            next.playback = StatusPlaybackMode::Fixed(id);
            carousel.update_status_playback(next)?;
            carousel.current = id;
            Ok::<_, anyhow::Error>(())
        }) {
            Ok(()) => self.close_picker(window, cx),
            Err(error) => {
                self.preview_error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }

    pub(super) fn preview_ready(&self, preview: &WallpaperPreview) -> bool {
        self.wallpaper.shown == preview.id
            && self
                .wallpaper
                .requested
                .is_some_and(|(id, _)| id == preview.id)
            && self.wallpaper.task.is_none()
            && self.wallpaper.ready.is_none()
            && self.preview_error.is_none()
    }
}

/// Size in device pixels at which the wallpaper is decoded for this window.
fn wallpaper_target(window: &Window) -> Size<u32> {
    let logical = window.viewport_size();
    let scale = window.scale_factor();
    Size::new(
        (f32::from(logical.width) * scale).ceil() as u32,
        (f32::from(logical.height) * scale).ceil() as u32,
    )
}
