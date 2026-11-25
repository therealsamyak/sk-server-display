//! Wallpaper gallery and slideshow editor.

mod status_picker_import;
mod status_picker_input;

use std::{
    collections::HashMap,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, ElementId, Entity, EventEmitter, FocusHandle,
    FontWeight, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, Pixels,
    Render, RenderImage, Role, ScrollHandle, SharedString, Size, Stateful, StyledImage as _,
    Window, div, img, point, prelude::*, px, rgb, rgba,
};

use crate::{
    status_fonts::STATUS_SANS_FONT_FAMILY,
    wallpaper::status_import::{ImportedWallpaper, WallpaperId},
    wallpaper::status_playlist::{PlaylistChoice, PlaylistSettings, StatusPlaylist},
    wallpaper::status_style::{ALL_STATUS_STYLES, StatusStyleName, status_style_by_name},
    wallpaper::status_wallpaper::load_status_wallpaper,
};

use self::status_picker_input::PickerInput;

const PICKER_BG: u32 = 0x111720;
const PICKER_SURFACE: u32 = 0x1b2430;
const PICKER_HOVER: u32 = 0x2a394a;
const PICKER_SELECTED: u32 = 0x344c60;
const PICKER_BORDER: u32 = 0x364454;
const PICKER_TEXT: u32 = 0xe8edf2;
const PICKER_MUTED: u32 = 0xa3b3c4;
const PICKER_FOCUS: u32 = 0xa4d9ff;
const PICKER_ERROR: u32 = 0xffb3ab;
const MINUTES_PER_HOUR: u32 = 60;
const MINUTES_PER_DAY: u32 = 24 * MINUTES_PER_HOUR;
const INTERVAL_FIELDS: [(&str, u32); 3] = [
    ("Days", u32::MAX / MINUTES_PER_DAY),
    ("Hours", 23),
    ("Minutes", MINUTES_PER_HOUR - 1),
];

/// User actions that affect the status display or saved slideshow settings.
pub enum StatusPickerEvent {
    /// Show a style and its wallpaper in full-screen preview.
    Preview {
        wallpaper: WallpaperId,
        draft: Option<ImportedWallpaper>,
    },
    /// Persist edits to Favorites and custom slideshows.
    Save {
        /// The full slideshow collection after the edit.
        settings: PlaylistSettings,
        /// The old and new names when the edit renames a custom slideshow.
        rename: Option<(String, String)>,
    },
    /// Start playing Favorites or one of the named slideshows.
    Play {
        /// Slideshow to start.
        choice: PlaylistChoice,
        /// Settings to persist with the playback selection.
        settings: PlaylistSettings,
    },
    /// Close the picker.
    Cancel,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PickerPage {
    Styles,
    Presets,
    Import,
    Slideshows,
    AddStyles,
}

#[derive(Clone, Copy)]
enum NameForm {
    Create,
    Rename,
}

enum WallpaperThumbnail {
    Ready(Arc<RenderImage>),
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ButtonTone {
    Normal,
    Selected,
    Primary,
    Disabled,
}

/// Native style picker overlay, with a gallery and a slideshow editor.
pub struct StatusPicker {
    active: WallpaperId,
    presets: Option<Arc<HashMap<StatusStyleName, PathBuf>>>,
    imports: Vec<ImportedWallpaper>,
    thumbnails: HashMap<WallpaperId, WallpaperThumbnail>,
    import: Option<status_picker_import::WallpaperImportForm>,
    settings: PlaylistSettings,
    page: PickerPage,
    playlist: PlaylistChoice,
    name_form: Option<NameForm>,
    focus: FocusHandle,
    name: Entity<PickerInput>,
    interval: [Entity<PickerInput>; 3],
    list_scroll: ScrollHandle,
    gallery_scroll: ScrollHandle,
    gallery_press: Option<(Pixels, Pixels)>,
    gallery_dragged: bool,
    validation: Option<&'static str>,
    save_error: Option<String>,
}

impl EventEmitter<StatusPickerEvent> for StatusPicker {}

impl StatusPicker {
    /// Opens the style picker and starts decoding wallpaper thumbnails on a background thread.
    pub fn new_status_picker(
        active: WallpaperId,
        presets: Option<Arc<HashMap<StatusStyleName, PathBuf>>>,
        imports: Vec<ImportedWallpaper>,
        settings: PlaylistSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let page = if presets.is_some() {
            PickerPage::Presets
        } else {
            PickerPage::Styles
        };
        let mut picker = Self {
            active,
            presets,
            imports,
            import: None,
            thumbnails: HashMap::new(),
            settings,
            page,
            playlist: PlaylistChoice::Favorites,
            name_form: None,
            focus,
            name: cx.new(|cx| PickerInput::new_picker_input("Slideshow name", "Name", false, cx)),
            interval: INTERVAL_FIELDS
                .map(|(label, _)| cx.new(|cx| PickerInput::new_picker_input(label, "0", true, cx))),
            list_scroll: ScrollHandle::new(),
            gallery_scroll: ScrollHandle::new(),
            gallery_press: None,
            gallery_dragged: false,
            validation: None,
            save_error: None,
        };
        picker.reset_interval(window, cx);
        picker.load_thumbnails(window, cx);
        cx.on_release_in(window, |picker, window, _| {
            for thumbnail in picker.thumbnails.values() {
                if let WallpaperThumbnail::Ready(image) = thumbnail
                    && let Err(error) = window.drop_image(image.clone())
                {
                    log::debug!("cannot release style thumbnail: {error:#}");
                }
            }
        })
        .detach();
        picker
    }

    /// Restores keyboard focus when returning from a full-screen preview.
    pub fn focus_status_picker(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }

    /// Shows a slideshow persistence error above the current picker page.
    pub fn set_save_error(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.save_error = error;
        cx.notify();
    }

    fn load_thumbnails(&self, window: &mut Window, cx: &mut Context<Self>) {
        let wallpapers: Vec<_> = self
            .imports
            .iter()
            .map(|entry| (WallpaperId::Imported(entry.id), entry.path.clone()))
            .chain(self.presets.iter().flat_map(|presets| {
                presets
                    .iter()
                    .map(|(&preset, path)| (WallpaperId::Preset(preset), path.clone()))
            }))
            .collect();
        cx.spawn_in(window, async move |picker, window| {
            for (style, path) in wallpapers {
                let thumbnail = window
                    .background_spawn(
                        async move { load_status_wallpaper(&path, Size::new(352, 198)) },
                    )
                    .await;
                let updated = picker.update_in(window, |picker, _, cx| {
                    let thumbnail = match thumbnail {
                        Ok(image) => WallpaperThumbnail::Ready(image),
                        Err(error) => {
                            log::warn!(
                                "cannot load thumbnail for {}: {error:#}",
                                style.wallpaper_key()
                            );
                            WallpaperThumbnail::Failed
                        }
                    };
                    picker.thumbnails.insert(style, thumbnail);
                    cx.notify();
                });
                if updated.is_err() {
                    return;
                }
            }
        })
        .detach();
    }

    fn imported_wallpaper(&self, style: WallpaperId) -> Option<&ImportedWallpaper> {
        match style {
            WallpaperId::Preset(_) => None,
            WallpaperId::Imported(id) => self.imports.iter().find(|entry| entry.id == id),
        }
    }

    fn wallpaper_path(&self, style: WallpaperId) -> Option<&Path> {
        match style {
            WallpaperId::Preset(preset) => {
                self.presets.as_ref()?.get(&preset).map(PathBuf::as_path)
            }
            WallpaperId::Imported(_) => self
                .imported_wallpaper(style)
                .map(|entry| entry.path.as_path()),
        }
    }

    fn preview_available(&self, style: WallpaperId) -> bool {
        style == WallpaperId::PLAIN
            || (self.wallpaper_path(style).is_some()
                && !matches!(
                    self.thumbnails.get(&style),
                    Some(WallpaperThumbnail::Failed)
                ))
    }

    fn wallpaper_title(&self, style: WallpaperId) -> String {
        self.imported_wallpaper(style)
            .map_or_else(|| style.wallpaper_title(), |entry| entry.name.clone())
    }

    fn selected_playlist(&self) -> Option<&StatusPlaylist> {
        self.settings.selected_playlist(&self.playlist)
    }

    fn selected_playlist_mut(&mut self) -> Option<&mut StatusPlaylist> {
        match &self.playlist {
            PlaylistChoice::Favorites => Some(&mut self.settings.favorites),
            PlaylistChoice::Custom(name) => self
                .settings
                .slideshows
                .iter_mut()
                .find(|playlist| &playlist.name == name),
        }
    }

    fn save(&self, rename: Option<(String, String)>, cx: &mut Context<Self>) {
        cx.emit(StatusPickerEvent::Save {
            settings: self.settings.clone(),
            rename,
        });
        cx.notify();
    }

    fn set_page(&mut self, page: PickerPage, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != page {
            self.gallery_scroll.set_offset(Default::default());
        }
        self.page = page;
        self.gallery_press = None;
        self.gallery_dragged = false;
        self.import = None;
        self.name_form = None;
        self.validation = None;
        self.focus_status_picker(window, cx);
        cx.notify();
    }

    fn select_playlist(&mut self, choice: PlaylistChoice, window: &Window, cx: &mut Context<Self>) {
        self.playlist = choice;
        self.name_form = None;
        self.validation = None;
        self.reset_interval(window, cx);
        self.list_scroll.set_offset(Default::default());
        cx.notify();
    }

    fn reset_interval(&mut self, window: &Window, cx: &mut Context<Self>) {
        if let Some(playlist) = self.selected_playlist() {
            let minutes = playlist.interval_minutes.get();
            let parts = [
                minutes / MINUTES_PER_DAY,
                minutes % MINUTES_PER_DAY / MINUTES_PER_HOUR,
                minutes % MINUTES_PER_HOUR,
            ];
            for (field, value) in self.interval.iter().zip(parts) {
                field.update(cx, |field, cx| {
                    field.set_picker_input(value.to_string(), window, cx)
                });
            }
        }
    }

    fn read_interval(&self, cx: &App) -> Option<NonZeroU32> {
        let [days, hours, minutes] = self.interval.each_ref().map(|field| {
            let text = field.read(cx).picker_input_text();
            if text.is_empty() {
                Some(0)
            } else {
                text.parse::<u32>().ok()
            }
        });
        let (days, hours, minutes) = (days?, hours?, minutes?);
        if hours >= 24 || minutes >= MINUTES_PER_HOUR {
            return None;
        }
        let minutes = days
            .checked_mul(MINUTES_PER_DAY)?
            .checked_add(hours * MINUTES_PER_HOUR + minutes)?;
        NonZeroU32::new(minutes)
    }

    fn apply_interval(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(interval) = self.read_interval(cx) else {
            self.validation = Some("Enter at least one minute. Use 0–23 hours and 0–59 minutes.");
            cx.notify();
            return false;
        };
        self.validation = None;
        let Some(playlist) = self.selected_playlist_mut() else {
            return false;
        };
        playlist.interval_minutes = interval;
        cx.notify();
        true
    }

    fn save_interval(&mut self, cx: &mut Context<Self>) {
        if self.apply_interval(cx) {
            self.save(None, cx);
        }
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        if self.apply_interval(cx) {
            cx.emit(StatusPickerEvent::Play {
                choice: self.playlist.clone(),
                settings: self.settings.clone(),
            });
        }
    }

    fn show_name_form(&mut self, form: NameForm, window: &mut Window, cx: &mut Context<Self>) {
        let name = match form {
            NameForm::Create => String::new(),
            NameForm::Rename => {
                let Some(playlist) = self.selected_playlist() else {
                    return;
                };
                playlist.name.clone()
            }
        };
        self.name_form = Some(form);
        self.validation = None;
        self.name.update(cx, |input, cx| {
            input.set_picker_input(name, window, cx);
            input.focus_picker_input(window, cx);
        });
        cx.notify();
    }

    fn save_name(&mut self, form: NameForm, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).picker_input_text().trim();
        if name.is_empty() {
            self.validation = Some("Enter a slideshow name.");
            cx.notify();
            return;
        }
        let normalized = name.to_lowercase();
        let taken = normalized == self.settings.favorites.name.to_lowercase()
            || self.settings.slideshows.iter().any(|playlist| {
                let same_list = matches!(form, NameForm::Rename)
                    && matches!(&self.playlist, PlaylistChoice::Custom(name) if name == &playlist.name);
                !same_list && playlist.name.to_lowercase() == normalized
            });
        if taken {
            self.validation = Some("A slideshow already uses this name.");
            cx.notify();
            return;
        }
        let name = name.to_owned();
        let rename = match form {
            NameForm::Create => {
                self.settings.slideshows.push(StatusPlaylist {
                    name: name.clone(),
                    styles: Vec::new(),
                    interval_minutes: NonZeroU32::new(60).unwrap_or(NonZeroU32::MIN),
                });
                None
            }
            NameForm::Rename => {
                let Some(playlist) = self.selected_playlist_mut() else {
                    return;
                };
                if playlist.name == name {
                    None
                } else {
                    let old_name = std::mem::replace(&mut playlist.name, name.clone());
                    Some((old_name, name.clone()))
                }
            }
        };
        self.select_playlist(PlaylistChoice::Custom(name), window, cx);
        self.focus_status_picker(window, cx);
        self.save(rename, cx);
    }

    fn toggle_style(&mut self, style: WallpaperId, cx: &mut Context<Self>) {
        let can_add = self.preview_available(style);
        let styles = match self.page {
            PickerPage::Styles | PickerPage::Presets => &mut self.settings.favorites.styles,
            PickerPage::AddStyles => {
                let Some(playlist) = self.selected_playlist_mut() else {
                    return;
                };
                &mut playlist.styles
            }
            PickerPage::Slideshows | PickerPage::Import => return,
        };
        if let Some(index) = styles.iter().position(|entry| *entry == style) {
            styles.remove(index);
        } else if can_add {
            styles.push(style);
        } else {
            return;
        }
        self.save(None, cx);
    }

    /// Moves a slideshow entry to another position in the play order (drag to reorder).
    fn move_style(&mut self, index: usize, destination: usize, cx: &mut Context<Self>) {
        if let Some(playlist) = self.selected_playlist_mut()
            && index != destination
            && index < playlist.styles.len()
            && destination < playlist.styles.len()
        {
            let style = playlist.styles.remove(index);
            playlist.styles.insert(destination, style);
            self.save(None, cx);
        }
    }

    fn remove_style(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(playlist) = self.selected_playlist_mut()
            && index < playlist.styles.len()
        {
            playlist.styles.remove(index);
            self.save(None, cx);
        }
    }

    fn scroll_gallery(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((start_y, start_offset)) = self.gallery_press.filter(|_| event.dragging()) else {
            return;
        };
        let delta = event.position.y - start_y;
        if self.gallery_dragged || delta.abs() >= px(12.) {
            self.gallery_dragged = true;
            let y = (start_offset + delta).clamp(-self.gallery_scroll.max_offset().y, px(0.));
            self.gallery_scroll.set_offset(point(px(0.), y));
            cx.notify();
        }
    }

    fn gallery_click_allowed(&self, event: &ClickEvent) -> bool {
        !self.gallery_dragged || matches!(event, ClickEvent::Keyboard(_))
    }

    fn gallery(&self, width: f32, cx: &mut Context<Self>) -> Div {
        let columns = if width >= 1060. {
            5
        } else if width >= 816. {
            4
        } else if width >= 612. {
            3
        } else if width >= 384. {
            2
        } else {
            1
        };
        let adding = self.page == PickerPage::AddStyles;
        let show_presets = self.page == PickerPage::Presets;
        let imported = self
            .imports
            .iter()
            .map(|entry| WallpaperId::Imported(entry.id));
        let presets = ALL_STATUS_STYLES
            .into_iter()
            .map(WallpaperId::Preset)
            .filter(|&id| {
                id != WallpaperId::PLAIN
                    && (show_presets || (adding && self.wallpaper_path(id).is_some()))
            });
        let entries = std::iter::once(WallpaperId::PLAIN)
            .chain(imported)
            .filter(|_| !show_presets)
            .chain(presets);
        let styles = if adding {
            self.selected_playlist()
                .map_or(&[][..], |playlist| playlist.styles.as_slice())
        } else {
            &self.settings.favorites.styles
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .when(adding, |body| {
                let name = self
                    .selected_playlist()
                    .map_or("", |playlist| playlist.name.as_str());
                body.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_5()
                        .pt_3()
                        .child(
                            div()
                                .text_lg()
                                .truncate()
                                .child(format!("Add wallpapers to {name}")),
                        )
                        .child(
                            button("finish-adding-styles", "Done", ButtonTone::Primary).on_click(
                                cx.listener(|picker, _, window, cx| {
                                    picker.set_page(PickerPage::Slideshows, window, cx);
                                }),
                            ),
                        ),
                )
            })
            .child(
                div()
                    .id("styles-gallery")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.gallery_scroll)
                    .capture_any_mouse_down(cx.listener(|picker, event: &MouseDownEvent, _, _| {
                        if event.button == MouseButton::Left {
                            picker.gallery_dragged = false;
                            picker.gallery_press =
                                Some((event.position.y, picker.gallery_scroll.offset().y));
                        }
                    }))
                    .on_mouse_move(
                        cx.listener(|picker, event, _, cx| picker.scroll_gallery(event, cx)),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|picker, _, _, _| picker.gallery_press = None),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|picker, _, _, _| picker.gallery_press = None),
                    )
                    .child(
                        div()
                            .grid()
                            .flex_none()
                            .grid_cols(columns)
                            .gap_3()
                            .p_5()
                            .children(entries.map(|style| {
                                self.style_card(style, styles.contains(&style), adding, cx)
                            })),
                    ),
            )
    }

    /// Wallpaper thumbnail at 16:9, or a status line while it is missing or loading.
    fn wallpaper_artwork(&self, style: WallpaperId) -> Div {
        let thumbnail = self.thumbnails.get(&style);
        let swatch = match style {
            WallpaperId::Preset(preset) => status_style_by_name(preset).palette,
            WallpaperId::Imported(_) => self.imported_wallpaper(style).map_or(
                status_style_by_name(StatusStyleName::Plain).palette,
                |entry| entry.wallpaper_style().palette,
            ),
        }
        .background
        .status_color();
        let artwork = div()
            .relative()
            .aspect_ratio(16. / 9.)
            .w_full()
            .overflow_hidden()
            .bg(swatch);
        match thumbnail {
            Some(WallpaperThumbnail::Ready(image)) => {
                artwork.child(img(image.clone()).size_full().object_fit(ObjectFit::Cover))
            }
            Some(WallpaperThumbnail::Failed) => {
                artwork.child(artwork_status("Cannot read wallpaper"))
            }
            None if self.wallpaper_path(style).is_some() => {
                artwork.child(artwork_status("Loading wallpaper"))
            }
            None if style == WallpaperId::PLAIN => {
                artwork.child(artwork_status("Solid background"))
            }
            None => artwork.child(artwork_status("Missing wallpaper")),
        }
    }

    fn style_card(
        &self,
        style: WallpaperId,
        in_list: bool,
        adding: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let title = self.wallpaper_title(style);
        let available = self.preview_available(style);
        let artwork = self
            .wallpaper_artwork(style)
            .id("preview-wallpaper-image")
            .when(available, |image| {
                image
                    .role(Role::Button)
                    .aria_label(format!("Preview {title}"))
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .focus_visible(|style| style.border_2().border_color(rgb(PICKER_FOCUS)))
                    .on_click(cx.listener(move |picker, event, _, cx| {
                        if picker.gallery_click_allowed(event) {
                            cx.emit(StatusPickerEvent::Preview {
                                wallpaper: style,
                                draft: None,
                            });
                        }
                    }))
            });
        let (list_label, list_action) = match (adding, in_list) {
            (true, true) => ("Remove", "Remove from slideshow"),
            (true, false) => ("Add", "Add to slideshow"),
            (false, true) => ("Favorited", "Remove from Favorites"),
            (false, false) => ("Favorite", "Add to Favorites"),
        };
        let list_tone = if in_list {
            ButtonTone::Selected
        } else if available {
            ButtonTone::Normal
        } else {
            ButtonTone::Disabled
        };
        div()
            .id(SharedString::from(style.wallpaper_key().into_owned()))
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .rounded_lg()
            .border_1()
            .border_color(rgb(PICKER_BORDER))
            .bg(rgb(PICKER_SURFACE))
            .child(artwork)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_3()
                    .pt_2()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(title.clone()),
                    )
                    .when(style == self.active, |heading| {
                        heading.child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(rgb(PICKER_FOCUS))
                                .child("Current"),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .p_2()
                    .child(
                        button("style-membership", list_label, list_tone)
                            .aria_label(format!("{list_action} · {title}"))
                            .aria_selected(in_list)
                            .when(available || in_list, |button| {
                                button.on_click(cx.listener(move |picker, event, _, cx| {
                                    if picker.gallery_click_allowed(event) {
                                        picker.toggle_style(style, cx);
                                    }
                                }))
                            }),
                    )
                    .when(!adding && self.imported_wallpaper(style).is_some(), |row| {
                        row.child(
                            button("edit-wallpaper", "Edit", ButtonTone::Normal)
                                .aria_label(format!("Edit {title}"))
                                .on_click(cx.listener(move |picker, event, window, cx| {
                                    if picker.gallery_click_allowed(event) {
                                        picker.open_import_form(Some(style), window, cx);
                                    }
                                })),
                        )
                    }),
            )
    }

    fn slideshows(&self, compact: bool, cx: &mut Context<Self>) -> Div {
        let navigation =
            div()
                .id("slideshow-list")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap_2()
                .p_3()
                .overflow_y_scroll()
                .children(
                    std::iter::once((PlaylistChoice::Favorites, &self.settings.favorites))
                        .chain(self.settings.slideshows.iter().map(|playlist| {
                            (PlaylistChoice::Custom(playlist.name.clone()), playlist)
                        }))
                        .map(|(choice, playlist)| {
                            let selected = choice == self.playlist
                                && !matches!(self.name_form, Some(NameForm::Create));
                            button(
                                format!("slideshow-{}", playlist.name),
                                playlist.name.clone(),
                                selected_tone(selected),
                            )
                            .aria_selected(selected)
                            .justify_start()
                            .on_click(cx.listener(
                                move |picker, _, window, cx| {
                                    picker.select_playlist(choice.clone(), window, cx);
                                },
                            ))
                        }),
                );
        let editor = div()
            .id("slideshow-editor")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .p_5()
            .children(if let Some(form) = self.name_form {
                Some(self.name_editor(form, cx))
            } else {
                self.selected_playlist()
                    .map(|playlist| self.playlist_editor(playlist, if compact { 2 } else { 4 }, cx))
            });

        div()
            .flex()
            .when(compact, |body| body.flex_col())
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .min_h_0()
                    .when(compact, |nav| nav.h(px(188.)).border_b_1())
                    .when(!compact, |nav| nav.w(px(224.)).border_r_1())
                    .border_color(rgb(PICKER_BORDER))
                    .child(
                        div().px_3().pt_3().child(
                            button("new-slideshow", "New slideshow", ButtonTone::Normal)
                                .w_full()
                                .justify_start()
                                .on_click(cx.listener(|picker, _, window, cx| {
                                    picker.show_name_form(NameForm::Create, window, cx);
                                })),
                        ),
                    )
                    .child(navigation),
            )
            .child(editor)
    }

    fn name_editor(&self, form: NameForm, cx: &mut Context<Self>) -> Div {
        let (title, action) = match form {
            NameForm::Create => ("New slideshow", "Create"),
            NameForm::Rename => ("Rename slideshow", "Save name"),
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .max_w(px(480.))
            .child(picker_heading(title))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(PICKER_MUTED))
                    .child("Slideshow name"),
            )
            .child(self.name.clone())
            .children(self.validation.map(validation_notice))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("save-slideshow-name", action, ButtonTone::Primary).on_click(
                            cx.listener(move |picker, _, window, cx| {
                                picker.save_name(form, window, cx);
                            }),
                        ),
                    )
                    .child(
                        button("cancel-name-form", "Cancel", ButtonTone::Normal).on_click(
                            cx.listener(|picker, _, window, cx| {
                                picker.name_form = None;
                                picker.validation = None;
                                picker.focus_status_picker(window, cx);
                                cx.notify();
                            }),
                        ),
                    ),
            )
    }

    fn playlist_editor(
        &self,
        playlist: &StatusPlaylist,
        columns: u16,
        cx: &mut Context<Self>,
    ) -> Div {
        let can_play = playlist
            .styles
            .iter()
            .any(|style| self.preview_available(*style));
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(picker_heading(playlist.name.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .when(self.playlist != PlaylistChoice::Favorites, |row| {
                                row.child(
                                    button("rename-slideshow", "Rename", ButtonTone::Normal)
                                        .on_click(cx.listener(|picker, _, window, cx| {
                                            picker.show_name_form(NameForm::Rename, window, cx);
                                        })),
                                )
                            })
                            .child(
                                button(
                                    "play-slideshow",
                                    "Play",
                                    if can_play {
                                        ButtonTone::Primary
                                    } else {
                                        ButtonTone::Disabled
                                    },
                                )
                                .when(can_play, |button| {
                                    button.on_click(cx.listener(|picker, _, _, cx| picker.play(cx)))
                                }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Time per wallpaper"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .gap_2()
                            .children(INTERVAL_FIELDS.iter().enumerate().map(
                                |(index, &(label, limit))| {
                                    self.interval_field(index, label, limit, cx)
                                },
                            ))
                            .child(
                                button(
                                    "save-slideshow-interval",
                                    "Save interval",
                                    ButtonTone::Normal,
                                )
                                .on_click(cx.listener(
                                    |picker, _, _, cx| {
                                        picker.save_interval(cx);
                                    },
                                )),
                            ),
                    )
                    .children(self.validation.map(validation_notice)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(PICKER_BORDER))
                    .pt_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .child(
                                        div().font_weight(FontWeight::SEMIBOLD).child("Play order"),
                                    )
                                    .when(playlist.styles.len() > 1, |heading| {
                                        heading.child(
                                            div()
                                                .truncate()
                                                .text_color(rgb(PICKER_MUTED))
                                                .child("Drag to reorder"),
                                        )
                                    }),
                            )
                            .child(
                                button(
                                    "add-slideshow-styles",
                                    "Add wallpapers",
                                    ButtonTone::Normal,
                                )
                                .on_click(cx.listener(
                                    |picker, _, window, cx| {
                                        picker.set_page(PickerPage::AddStyles, window, cx);
                                    },
                                )),
                            ),
                    )
                    .when(playlist.styles.is_empty(), |list| {
                        list.child(
                            div()
                                .text_color(rgb(PICKER_MUTED))
                                .child("Add wallpapers to start this slideshow."),
                        )
                    })
                    .child(
                        div().grid().grid_cols(columns).gap_3().children(
                            playlist
                                .styles
                                .iter()
                                .enumerate()
                                .map(|(index, &style)| self.playlist_tile(index, style, cx)),
                        ),
                    ),
            )
    }

    fn interval_field(
        &self,
        index: usize,
        label: &'static str,
        limit: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(label)
            .flex()
            .flex_col()
            .gap_1()
            .w(px(96.))
            .child(div().text_sm().text_color(rgb(PICKER_MUTED)).child(label))
            .child(self.interval[index].clone())
            .child(
                div().flex().justify_between().gap_1().children(
                    [("Decrease", "−"), ("Increase", "+")]
                        .into_iter()
                        .map(|(action, glyph)| {
                            button(action, glyph, ButtonTone::Normal)
                                .w(px(46.))
                                .aria_label(format!("{action} {}", label.to_lowercase()))
                                .on_click(cx.listener(move |picker, _, window, cx| {
                                    picker.interval[index].update(cx, |field, cx| {
                                        let previous =
                                            field.picker_input_text().parse::<u32>().unwrap_or(0);
                                        let value = if glyph == "+" {
                                            previous.saturating_add(1)
                                        } else {
                                            previous.saturating_sub(1)
                                        }
                                        .min(limit);
                                        field.set_picker_input(value.to_string(), window, cx);
                                    });
                                }))
                        }),
                ),
            )
    }

    fn playlist_tile(
        &self,
        index: usize,
        style: WallpaperId,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let title = self.wallpaper_title(style);
        let dragged = DraggedPlaylistTile {
            index,
            title: title.clone(),
            image: match self.thumbnails.get(&style) {
                Some(WallpaperThumbnail::Ready(image)) => Some(image.clone()),
                _ => None,
            },
        };
        div()
            .id(SharedString::from(style.wallpaper_key().into_owned()))
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .rounded_lg()
            .border_1()
            .border_color(rgb(PICKER_BORDER))
            .bg(rgb(PICKER_SURFACE))
            .cursor_grab()
            .on_drag(dragged, |tile, _, _, cx| cx.new(|_| tile.clone()))
            .drag_over::<DraggedPlaylistTile>(|style, _, _, _| {
                style.border_color(rgb(PICKER_FOCUS)).bg(rgb(PICKER_HOVER))
            })
            .on_drop(
                cx.listener(move |picker, tile: &DraggedPlaylistTile, _, cx| {
                    picker.move_style(tile.index, index, cx);
                }),
            )
            .child(self.wallpaper_artwork(style))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .child(
                        div()
                            .flex_none()
                            .text_color(rgb(PICKER_MUTED))
                            .child((index + 1).to_string()),
                    )
                    .child(div().min_w_0().truncate().child(title.clone())),
            )
            .child(
                div()
                    .id("remove-from-slideshow")
                    .role(Role::Button)
                    .aria_label(format!("Remove {title}"))
                    .absolute()
                    .top_1()
                    .right_1()
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(PICKER_BORDER))
                    .bg(rgb(PICKER_BG))
                    .text_color(rgb(PICKER_TEXT))
                    .cursor_pointer()
                    .hover(|button| button.bg(rgb(PICKER_HOVER)))
                    .on_click(cx.listener(move |picker, _, _, cx| picker.remove_style(index, cx)))
                    .child("×"),
            )
    }
}

/// Slideshow tile being dragged to reorder the play order, also shown as the drag preview.
#[derive(Clone)]
struct DraggedPlaylistTile {
    index: usize,
    title: String,
    image: Option<Arc<RenderImage>>,
}

impl Render for DraggedPlaylistTile {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let preview = div()
            .w(px(160.))
            .aspect_ratio(16. / 9.)
            .overflow_hidden()
            .rounded_md()
            .border_2()
            .border_color(rgb(PICKER_FOCUS))
            .bg(rgb(PICKER_SELECTED))
            .opacity(0.9);
        match &self.image {
            Some(image) => {
                preview.child(img(image.clone()).size_full().object_fit(ObjectFit::Cover))
            }
            None => preview
                .flex()
                .items_center()
                .justify_center()
                .px_2()
                .text_color(rgb(PICKER_TEXT))
                .font_family(STATUS_SANS_FONT_FAMILY)
                .text_sm()
                .child(div().truncate().child(self.title.clone())),
        }
    }
}

impl Render for StatusPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let compact = viewport.width < px(720.);
        let inset = if compact || viewport.height < px(520.) {
            8.
        } else {
            36.
        };
        let width = (f32::from(viewport.width) - 2. * inset).clamp(1., 1232.);
        div()
            .id("status-picker-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x070b13bd))
            .role(Role::Dialog)
            .aria_label("Wallpaper picker")
            .track_focus(&self.focus)
            .tab_group()
            .on_key_down(cx.listener(|picker, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" if picker.page == PickerPage::Import => {
                        picker.set_page(PickerPage::Styles, window, cx)
                    }
                    "escape" => cx.emit(StatusPickerEvent::Cancel),
                    "tab" if event.keystroke.modifiers.shift => window.focus_prev(cx),
                    "tab" => window.focus_next(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(width))
                    .h((viewport.height - px(2. * inset)).max(px(1.)))
                    .min_h_0()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(PICKER_BORDER))
                    .bg(rgb(PICKER_BG))
                    .overflow_hidden()
                    .font_family(STATUS_SANS_FONT_FAMILY)
                    .text_size(px(15.))
                    .line_height(px(20.))
                    .text_color(rgb(PICKER_TEXT))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_5()
                            .py_3()
                            .border_b_1()
                            .border_color(rgb(PICKER_BORDER))
                            .when(self.page != PickerPage::Import, |header| {
                                header.child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap_2()
                                        .when(self.presets.is_some(), |tabs| {
                                            tabs.child(
                                                button(
                                                    "presets-tab",
                                                    "Presets",
                                                    selected_tone(self.page == PickerPage::Presets),
                                                )
                                                .on_click(cx.listener(|picker, _, window, cx| {
                                                    picker.set_page(PickerPage::Presets, window, cx)
                                                })),
                                            )
                                        })
                                        .child(
                                            button(
                                                "slideshows-tab",
                                                "Slideshows",
                                                selected_tone(matches!(
                                                    self.page,
                                                    PickerPage::Slideshows | PickerPage::AddStyles
                                                )),
                                            )
                                            .on_click(
                                                cx.listener(|picker, _, window, cx| {
                                                    picker.set_page(
                                                        PickerPage::Slideshows,
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                            ),
                                        )
                                        .child(
                                            button(
                                                "styles-tab",
                                                "Wallpapers",
                                                selected_tone(self.page == PickerPage::Styles),
                                            )
                                            .on_click(
                                                cx.listener(|picker, _, window, cx| {
                                                    picker.set_page(PickerPage::Styles, window, cx);
                                                }),
                                            ),
                                        ),
                                )
                            })
                            .when(self.page == PickerPage::Import, |header| {
                                header.child(picker_heading(
                                    if self.import.as_ref().is_some_and(|form| form.is_edit()) {
                                        "Edit wallpaper"
                                    } else {
                                        "Import wallpaper"
                                    },
                                ))
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .when(self.page == PickerPage::Styles, |actions| {
                                        actions.child(
                                            button(
                                                "import-wallpaper",
                                                "Import image",
                                                ButtonTone::Primary,
                                            )
                                            .on_click(
                                                cx.listener(|picker, _, window, cx| {
                                                    picker.open_import_form(None, window, cx)
                                                }),
                                            ),
                                        )
                                    })
                                    .child(
                                        button("close-style-picker", "Cancel", ButtonTone::Normal)
                                            .on_click(cx.listener(|picker, _, window, cx| {
                                                if picker.page == PickerPage::Import {
                                                    picker.set_page(PickerPage::Styles, window, cx);
                                                } else {
                                                    cx.emit(StatusPickerEvent::Cancel);
                                                }
                                            })),
                                    ),
                            ),
                    )
                    .children(
                        self.save_error.as_ref().map(|error| {
                            div().px_5().py_2().child(validation_notice(error.clone()))
                        }),
                    )
                    .child(match self.page {
                        PickerPage::Styles | PickerPage::Presets | PickerPage::AddStyles => {
                            self.gallery(width, cx)
                        }
                        PickerPage::Import => self.import_editor(cx),
                        PickerPage::Slideshows => self.slideshows(compact, cx),
                    }),
            )
    }
}

fn artwork_status(label: &'static str) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .bg(rgb(PICKER_BG))
                .text_color(rgb(PICKER_MUTED))
                .text_xs()
                .rounded_md()
                .px_2()
                .py_1()
                .child(label),
        )
}

fn picker_heading(label: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .max_w_full()
        .truncate()
        .text_size(px(24.))
        .line_height(px(30.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label.into())
}

fn validation_notice(label: impl Into<SharedString>) -> Div {
    div().text_color(rgb(PICKER_ERROR)).child(label.into())
}

const fn selected_tone(selected: bool) -> ButtonTone {
    if selected {
        ButtonTone::Selected
    } else {
        ButtonTone::Normal
    }
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tone: ButtonTone,
) -> Stateful<Div> {
    let label = label.into();
    let (background, ink) = match tone {
        ButtonTone::Normal => (PICKER_SURFACE, PICKER_TEXT),
        ButtonTone::Selected => (PICKER_SELECTED, PICKER_TEXT),
        ButtonTone::Primary => (PICKER_FOCUS, PICKER_BG),
        ButtonTone::Disabled => (PICKER_BG, PICKER_MUTED),
    };
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .flex()
        .h(px(44.))
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(PICKER_BORDER))
        .bg(rgb(background))
        .text_color(rgb(ink))
        .text_sm()
        .px_3()
        .child(div().truncate().child(label))
        .when(tone != ButtonTone::Disabled, |button| {
            let hover = if tone == ButtonTone::Primary {
                PICKER_TEXT
            } else {
                PICKER_HOVER
            };
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(move |style| style.bg(rgb(hover)))
                .focus_visible(|style| style.border_color(rgb(PICKER_FOCUS)))
        })
}
