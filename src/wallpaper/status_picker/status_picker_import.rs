//! Import editor for a local image and its panel layout.

use std::{num::NonZeroU32, path::PathBuf};

use anyhow::Context as _;
use gpui::{
    App, AppContext as _, Context, Div, Entity, PathPromptOptions, Styled as _, Window, div,
    prelude::*, px, rgb,
};

use super::{
    ButtonTone, PICKER_MUTED, PickerInput, PickerPage, StatusPicker, StatusPickerEvent, button,
    selected_tone,
};
use crate::wallpaper::status_import::{
    ImportedWallpaper, PanelAppearance, PanelPlacement, WallpaperId,
};

/// Unsaved inputs kept when you return from the wallpaper preview.
pub(super) struct WallpaperImportForm {
    existing: Option<NonZeroU32>,
    path: Entity<PickerInput>,
    name: Entity<PickerInput>,
    placement: PanelPlacement,
    appearance: PanelAppearance,
}

impl WallpaperImportForm {
    /// True when the form edits a saved wallpaper.
    pub(super) fn is_edit(&self) -> bool {
        self.existing.is_some()
    }

    fn draft(&self, imports: &[ImportedWallpaper], cx: &App) -> anyhow::Result<ImportedWallpaper> {
        let id = match self.existing {
            Some(id) => id,
            None => ImportedWallpaper::next_import_id(imports)?,
        };
        let path = self.path.read(cx).picker_input_text();
        anyhow::ensure!(
            !path.is_empty(),
            "choose a PNG or JPEG file or enter its path"
        );
        let name = self.name.read(cx).picker_input_text().trim();
        let draft = ImportedWallpaper {
            id,
            name: name.to_owned(),
            path: std::path::absolute(path).context("cannot resolve image path")?,
            placement: self.placement,
            appearance: self.appearance,
        };
        draft.validate_import()?;
        Ok(draft)
    }
}

impl StatusPicker {
    /// Open the editor with a blank import or the selected wallpaper's settings.
    pub(super) fn open_import_form(
        &mut self,
        wallpaper: Option<WallpaperId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = wallpaper
            .and_then(|id| self.imported_wallpaper(id))
            .cloned();
        self.set_page(PickerPage::Import, window, cx);
        let path = cx
            .new(|cx| PickerInput::new_picker_input("Image file", "/path/to/image.jpg", false, cx));
        let name =
            cx.new(|cx| PickerInput::new_picker_input("Wallpaper name", "Display name", false, cx));
        if let Some(entry) = &existing {
            path.update(cx, |path, cx| {
                path.set_picker_input(entry.path.to_string_lossy().into_owned(), window, cx)
            });
            name.update(cx, |name, cx| {
                name.set_picker_input(entry.name.clone(), window, cx)
            });
        }
        self.import = Some(WallpaperImportForm {
            existing: existing.as_ref().map(|entry| entry.id),
            path,
            name,
            placement: existing
                .as_ref()
                .map(|entry| entry.placement)
                .unwrap_or_default(),
            appearance: existing
                .as_ref()
                .map(|entry| entry.appearance)
                .unwrap_or_default(),
        });
    }

    fn browse_for_wallpaper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let response = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose wallpaper".into()),
        });
        cx.spawn_in(window, async move |picker, window| {
            let chosen = response
                .await
                .context("file picker closed")
                .and_then(|result| result);
            if let Err(error) = picker.update_in(window, |picker, window, cx| {
                let Some(form) = &picker.import else { return };
                match chosen {
                    Ok(Some(paths)) => {
                        if let Some(path) = paths.into_iter().next()
                            && let Err(error) = picker.accept_wallpaper_path(path, window, cx)
                        {
                            picker.set_save_error(Some(format!("{error:#}")), cx);
                        }
                    }
                    Ok(None) => form
                        .path
                        .update(cx, |path, cx| path.focus_picker_input(window, cx)),
                    Err(error) => picker.set_save_error(
                        Some(format!(
                            "Could not open the file picker. Enter the image path. {error:#}"
                        )),
                        cx,
                    ),
                }
            }) {
                log::debug!("wallpaper picker closed: {error:#}");
            }
        })
        .detach();
    }

    fn accept_wallpaper_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let form = self.import.as_ref().context("wallpaper editor closed")?;
        let text = path
            .to_str()
            .context("image path contains unsupported characters")?;
        form.path.update(cx, |path, cx| {
            path.set_picker_input(text.to_owned(), window, cx)
        });
        if form.name.read(cx).picker_input_text().is_empty() {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Wallpaper");
            form.name.update(cx, |input, cx| {
                input.set_picker_input(name.to_owned(), window, cx)
            });
        }
        form.name
            .update(cx, |name, cx| name.focus_picker_input(window, cx));
        self.set_save_error(None, cx);
        Ok(())
    }

    fn preview_import(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.import else { return };
        match form.draft(&self.imports, cx) {
            Ok(draft) => {
                self.set_save_error(None, cx);
                cx.emit(StatusPickerEvent::Preview {
                    wallpaper: WallpaperId::Imported(draft.id),
                    draft: Some(draft),
                });
            }
            Err(error) => self.set_save_error(Some(format!("{error:#}")), cx),
        }
    }

    /// Render the import fields and the action to preview their combined effect.
    pub(super) fn import_editor(&self, cx: &mut Context<Self>) -> Div {
        let Some(form) = &self.import else {
            return div();
        };
        let placements = [
            (PanelPlacement::Bottom, "Bottom"),
            (PanelPlacement::Left, "Left"),
            (PanelPlacement::Right, "Right"),
            (PanelPlacement::Split, "Split"),
        ];
        let appearances = [
            (PanelAppearance::Glass, "Glass"),
            (PanelAppearance::Opaque, "Opaque"),
        ];
        div()
            .flex()
            .flex_col()
            .min_h_0()
            .flex_1()
            .child(
                div()
                    .id("wallpaper-editor-fields")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_5()
                    .child(
                        div()
                            .max_w(px(680.))
                            .flex()
                            .flex_col()
                            .gap_5()
                            .child(
                                field_label("Image file")
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .min_w(px(180.))
                                                    .flex_1()
                                                    .child(form.path.clone()),
                                            )
                                            .child(
                                                button(
                                                    "browse-wallpaper",
                                                    "Browse…",
                                                    ButtonTone::Normal,
                                                )
                                                .on_click(cx.listener(|picker, _, window, cx| {
                                                    picker.browse_for_wallpaper(window, cx)
                                                })),
                                            ),
                                    )
                                    .child(div().text_sm().text_color(rgb(PICKER_MUTED)).child(
                                        "Choose a local PNG or JPEG. Keep the file at this path.",
                                    )),
                            )
                            .child(field_label("Wallpaper name").child(form.name.clone()))
                            .child(field_label("Panel placement").child(
                                div().flex().flex_wrap().gap_2().children(placements.map(
                                    |(placement, label)| {
                                        let selected = form.placement == placement;
                                        button(
                                            (gpui::ElementId::from("panel-placement"), label),
                                            label,
                                            selected_tone(selected),
                                        )
                                        .aria_selected(selected)
                                        .on_click(
                                            cx.listener(move |picker, _, _, cx| {
                                                if let Some(form) = &mut picker.import {
                                                    form.placement = placement;
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                    },
                                )),
                            ))
                            .child(field_label("Panel appearance").child(
                                div().flex().flex_wrap().gap_2().children(appearances.map(
                                    |(appearance, label)| {
                                        let selected = form.appearance == appearance;
                                        button(
                                            (gpui::ElementId::from("panel-appearance"), label),
                                            label,
                                            selected_tone(selected),
                                        )
                                        .aria_selected(selected)
                                        .on_click(
                                            cx.listener(move |picker, _, _, cx| {
                                                if let Some(form) = &mut picker.import {
                                                    form.appearance = appearance;
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                    },
                                )),
                            )),
                    ),
            )
            .child(
                div().flex().justify_end().p_5().child(
                    button("preview-import", "Preview wallpaper", ButtonTone::Primary)
                        .on_click(cx.listener(|picker, _, _, cx| picker.preview_import(cx))),
                ),
            )
    }
}

fn field_label(label: &'static str) -> Div {
    div().flex().flex_col().gap_2().child(div().child(label))
}
