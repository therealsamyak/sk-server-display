//! Top controls and their shared pointer proximity area.

use std::{cell::Cell, rc::Rc};

use gpui::{
    Bounds, Context, Div, HoverListenerMode, Pixels, Role, Window, div, prelude::*, relative,
};

use super::{
    NextStatusDisplay, QuitStatusDisplay, RefreshStatus, StatusView, ToggleFullscreen,
    status_view_layout::{SCREEN_PADDING, control_button},
};
use crate::dashboard::status_theme::{Scale, StatusTheme};

const TOOLBAR_HOVER_MARGIN: f32 = 12.;
const DISPLAY_SELECTOR_CLEARANCE: f32 = 110.;

impl StatusView {
    pub(super) fn status_toolbar(
        &self,
        s: Scale,
        theme: StatusTheme,
        window: &Window,
        show_window_controls: bool,
        cx: &Context<Self>,
    ) -> Div {
        let hover_bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
        let center_bounds = hover_bounds.clone();
        let right_bounds = hover_bounds.clone();
        let margin = s(TOOLBAR_HOVER_MARGIN);
        let old_bounds = self.toolbar_bounds;
        let view = cx.weak_entity();
        let visible = self.toolbar_hovered;
        let right = div()
            .flex()
            .items_center()
            .min_w_0()
            .gap(s(8.))
            .when(show_window_controls, |row| {
                row.child(
                    self.window_controls(s, theme, window, cx)
                        .when(!visible, |controls| controls.invisible()),
                )
            })
            .child(
                control_button(
                    s,
                    theme,
                    "open-wallpaper-picker",
                    "Wallpaper",
                    cx.listener(|view, _, window, cx| view.open_picker(window, cx)),
                )
                .flex_none(),
            );
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_children_prepainted(move |_, window, cx| {
                let bounds = hover_bounds.get().map(|area| area.dilate(margin));
                if bounds != old_bounds {
                    let view = view.clone();
                    window.defer(cx, move |_, cx| {
                        if let Err(error) = view.update(cx, |this, cx| {
                            this.toolbar_bounds = bounds;
                            cx.notify();
                        }) {
                            log::debug!("status toolbar closed: {error}");
                        }
                    });
                }
            })
            .children(old_bounds.map(|area| {
                div()
                    .id("status-toolbar-hover")
                    .absolute()
                    .left(area.origin.x)
                    .top(area.origin.y)
                    .w(area.size.width)
                    .h(area.size.height)
                    .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if this.toolbar_hovered != *hovered {
                            this.toolbar_hovered = *hovered;
                            cx.notify();
                        }
                    }))
            }))
            .when(show_window_controls, |toolbar| {
                toolbar.child(
                    div()
                        .absolute()
                        .top(s(SCREEN_PADDING))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .on_children_prepainted(move |bounds, _, _| {
                            center_bounds.set(bounds.first().copied());
                        })
                        .child(
                            div()
                                .id("control-display")
                                .role(Role::Button)
                                .aria_label("Cycle displays")
                                .cursor_pointer()
                                .text_size(s(13.))
                                .text_color(theme.muted())
                                .px(s(8.))
                                .py(s(4.))
                                .rounded(s(6.))
                                .bg(theme.panel())
                                .hover(move |style| style.text_color(theme.text()))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.next_display(&NextStatusDisplay, window, cx)
                                }))
                                .child(self.settings.display_label.clone())
                                .when(!visible, |selector| selector.invisible()),
                        ),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(s(SCREEN_PADDING))
                    .left(relative(0.5))
                    .ml(s(DISPLAY_SELECTOR_CLEARANCE))
                    .right(s(SCREEN_PADDING))
                    .flex()
                    .justify_end()
                    .on_children_prepainted(move |bounds, _, _| {
                        if let Some(right) = bounds.first() {
                            right_bounds.set(Some(
                                right_bounds
                                    .get()
                                    .map_or(*right, |center| center.union(right)),
                            ));
                        }
                    })
                    .child(right),
            )
    }

    fn window_controls(
        &self,
        s: Scale,
        theme: StatusTheme,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_wrap()
            .justify_end()
            .items_center()
            .gap(s(8.))
            .child(control_button(
                s,
                theme,
                "control-fullscreen",
                if window.is_fullscreen() {
                    "Windowed"
                } else {
                    "Fullscreen"
                },
                cx.listener(|view, _, window, cx| {
                    view.toggle_fullscreen(&ToggleFullscreen, window, cx)
                }),
            ))
            .child(control_button(
                s,
                theme,
                "control-keep-awake",
                if self.settings.keep_awake {
                    "Allow sleep"
                } else {
                    "Stay awake"
                },
                cx.listener(|view, _, _, cx| view.toggle_keep_awake(cx)),
            ))
            .child(control_button(
                s,
                theme,
                "control-refresh",
                "Refresh",
                cx.listener(|view, _, window, cx| view.refresh(&RefreshStatus, window, cx)),
            ))
            .child(
                control_button(
                    s,
                    theme,
                    "control-update",
                    concat!("v", env!("CARGO_PKG_VERSION")),
                    cx.listener(|view, _, window, cx| view.check_for_updates(window, cx)),
                )
                .aria_label(concat!(
                    "Check for updates. Version ",
                    env!("CARGO_PKG_VERSION")
                )),
            )
            .child(control_button(
                s,
                theme,
                "control-quit",
                "Quit",
                cx.listener(|view, _, window, cx| view.quit(&QuitStatusDisplay, window, cx)),
            ))
    }
}
