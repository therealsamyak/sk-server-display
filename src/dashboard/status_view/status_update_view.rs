//! Check for a GitHub release and present its result in the dashboard.

use std::process::ChildStdin;

use gpui::{AppContext as _, Context, Div, FontWeight, Global, Window, div, prelude::*, px};

use crate::{
    platform::{
        status_release::{ReleaseCheck, ReleaseUpdate, check_display_release},
        status_update::prepare_app_update,
    },
    status_fonts::STATUS_SANS_FONT_FAMILY,
    wallpaper::plain_status_style,
};

use super::{Scale, StatusTheme, StatusView, status_view_layout::control_button};

// The exit signal stays open until the application has shut down.
struct UpdateExitSignal(#[allow(dead_code)] ChildStdin);

impl Global for UpdateExitSignal {}

/// Available update, progress, or result to show in the dashboard.
pub(super) enum StatusUpdateDialog {
    Available(ReleaseUpdate),
    Downloading,
    Notice {
        title: &'static str,
        detail: Option<String>,
    },
}

impl StatusView {
    pub(super) fn check_for_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_busy {
            return;
        }
        self.update_busy = true;
        cx.notify();
        let check = cx.background_spawn(async { check_display_release() });
        cx.spawn_in(window, async move |view, cx| {
            let result = check.await;
            let _ = view.update_in(cx, |view, _, cx| {
                view.update_busy = false;
                view.update_dialog = Some(match result {
                    Ok(ReleaseCheck::Available(update)) => StatusUpdateDialog::Available(update),
                    Ok(ReleaseCheck::Unavailable) => StatusUpdateDialog::Notice {
                        title: "No public release is available",
                        detail: None,
                    },
                    Ok(ReleaseCheck::Current) => StatusUpdateDialog::Notice {
                        title: "You're up to date",
                        detail: Some(format!("Installed version {}", env!("CARGO_PKG_VERSION"))),
                    },
                    Err(error) => StatusUpdateDialog::Notice {
                        title: "Could not check for updates",
                        detail: Some(format!("{error:#}")),
                    },
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn close_update_dialog(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.update_dialog, Some(StatusUpdateDialog::Downloading)) {
            self.update_dialog = None;
            cx.notify();
        }
    }

    fn install_release(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(StatusUpdateDialog::Available(update)) = self.update_dialog.take() else {
            return;
        };
        self.update_dialog = Some(StatusUpdateDialog::Downloading);
        cx.notify();
        let transfer = cx.background_spawn(async move { prepare_app_update(&update) });
        cx.spawn_in(window, async move |view, cx| {
            let prepared = transfer.await;
            let _ = view.update_in(cx, |view, _, cx| {
                match prepared.and_then(|update| update.start_update_worker()) {
                    Ok(signal) => {
                        cx.set_global(UpdateExitSignal(signal));
                        cx.quit();
                    }
                    Err(error) => {
                        view.update_dialog = Some(StatusUpdateDialog::Notice {
                            title: "Could not install the update",
                            detail: Some(format!("{error:#}")),
                        });
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn render_update_dialog(
        &self,
        dialog: &StatusUpdateDialog,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = StatusTheme(plain_status_style().palette);
        let s: Scale = &px;
        let (title, detail, close_label) = match dialog {
            StatusUpdateDialog::Available(update) => (
                format!("Version {} is available", update.version),
                Some("Download the build for this device and restart the app.".into()),
                Some("Cancel"),
            ),
            StatusUpdateDialog::Downloading => ("Downloading update…".into(), None, None),
            StatusUpdateDialog::Notice { title, detail } => {
                ((*title).into(), detail.clone(), Some("OK"))
            }
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .bg(theme.background().opacity(0.68))
            .font_family(STATUS_SANS_FONT_FAMILY)
            .text_color(theme.text())
            .child(
                div()
                    .w_full()
                    .max_w(px(480.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.panel_border())
                    .bg(theme.background())
                    .child(
                        div()
                            .text_size(px(21.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .children(detail.map(|text| {
                        div()
                            .text_size(px(15.))
                            .text_color(theme.muted())
                            .child(text)
                    }))
                    .children(close_label.map(|label| {
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                control_button(
                                    s,
                                    theme,
                                    "close-update-dialog",
                                    label,
                                    cx.listener(|view, _, _, cx| view.close_update_dialog(cx)),
                                )
                                .h(px(44.)),
                            )
                            .when(
                                matches!(dialog, StatusUpdateDialog::Available(_)),
                                |actions| {
                                    actions.child(
                                        control_button(
                                            s,
                                            theme,
                                            "install-update",
                                            "Update and restart",
                                            cx.listener(|view, _, window, cx| {
                                                view.install_release(window, cx)
                                            }),
                                        )
                                        .h(px(44.))
                                        .border_color(theme.info().opacity(0.65))
                                        .text_color(theme.text()),
                                    )
                                },
                            )
                    })),
            )
    }
}
