//! Single-line text input for the style picker.

use std::ops::Range;

use gpui::{
    Bounds, ClickEvent, ClipboardItem, Context, CursorStyle, ElementInputHandler,
    EntityInputHandler, FocusHandle, IntoElement, KeyDownEvent, Pixels, Point, Render, Role,
    ShapedLine, SharedString, TextAlign, TextRun, UTF16Selection, Window, canvas, div, fill, point,
    prelude::*, px, rgb, size,
};

use super::{PICKER_BORDER, PICKER_FOCUS, PICKER_MUTED, PICKER_SURFACE, PICKER_TEXT};

pub(super) struct PickerInput {
    focus: FocusHandle,
    label: &'static str,
    placeholder: &'static str,
    numeric: bool,
    text: String,
    cursor: usize,
    anchor: usize,
    marked: Option<Range<usize>>,
    layout: Option<(ShapedLine, Point<Pixels>)>,
}

impl PickerInput {
    pub(super) fn new_picker_input(
        label: &'static str,
        placeholder: &'static str,
        numeric: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus: cx.focus_handle().tab_stop(true),
            label,
            placeholder,
            numeric,
            text: String::new(),
            cursor: 0,
            anchor: 0,
            marked: None,
            layout: None,
        }
    }

    pub(super) fn picker_input_text(&self) -> &str {
        &self.text
    }

    pub(super) fn set_picker_input(
        &mut self,
        text: String,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.text = text;
        self.cursor = self.text.len();
        self.anchor = self.cursor;
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    pub(super) fn focus_picker_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }

    fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }

    fn byte_offset(&self, index: usize) -> usize {
        let mut units = 0;
        for (offset, character) in self.text.char_indices() {
            if units >= index {
                return offset;
            }
            units += character.len_utf16();
        }
        self.text.len()
    }

    fn utf16_offset(&self, index: usize) -> usize {
        self.text[..index].encode_utf16().count()
    }

    fn utf8_range(&self, range: Range<usize>) -> Range<usize> {
        self.byte_offset(range.start)..self.byte_offset(range.end.max(range.start))
    }

    fn utf16_range(&self, range: Range<usize>) -> Range<usize> {
        self.utf16_offset(range.start)..self.utf16_offset(range.end)
    }

    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Range<usize> {
        let range = range
            .map(|range| self.utf8_range(range))
            .or(self.marked.take())
            .unwrap_or_else(|| self.selection());
        let insertion = text
            .chars()
            .filter(|character| {
                if self.numeric {
                    character.is_ascii_digit()
                } else {
                    !character.is_control()
                }
            })
            .collect::<String>();
        let inserted = range.start..range.start + insertion.len();
        self.text.replace_range(range, &insertion);
        self.cursor = inserted.end;
        self.anchor = self.cursor;
        inserted
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let command = key.modifiers.control || key.modifiers.platform;
        let previous = self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(offset, _)| offset);
        let next = self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |character| self.cursor + character.len_utf8());
        let selection = self.selection();
        match (key.key.as_str(), command) {
            ("a", true) => {
                self.anchor = 0;
                self.cursor = self.text.len();
            }
            ("c" | "x", true) => {
                if !selection.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.text[selection].to_owned(),
                    ));
                    if key.key == "x" {
                        self.replace_text_in_range(None, "", window, cx);
                    }
                }
            }
            ("v", true) => {
                if let Some(item) = cx.read_from_clipboard() {
                    self.paste(item, window, cx);
                }
            }
            ("backspace" | "delete", false) => {
                if selection.is_empty() {
                    self.anchor = if key.key == "backspace" {
                        previous
                    } else {
                        next
                    };
                }
                self.replace_text_in_range(None, "", window, cx);
            }
            ("left" | "right" | "home" | "end", _) => {
                self.cursor = match key.key.as_str() {
                    "left" if command => 0,
                    "right" if command => self.text.len(),
                    "left" if !key.modifiers.shift && !selection.is_empty() => selection.start,
                    "right" if !key.modifiers.shift && !selection.is_empty() => selection.end,
                    "left" => previous,
                    "right" => next,
                    "home" => 0,
                    _ => self.text.len(),
                };
                if !key.modifiers.shift {
                    self.anchor = self.cursor;
                }
            }
            _ => return,
        }
        window.notify_text_selection_changed(&self.focus);
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn click(&mut self, event: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_picker_input(window, cx);
        window.request_virtual_keyboard();
        if !matches!(event, ClickEvent::Keyboard(_))
            && let Some((line, origin)) = &self.layout
        {
            let index = line.closest_index_for_x(event.position().x - origin.x);
            self.cursor = self.text.floor_char_boundary(index);
            if !event.modifiers().shift {
                self.anchor = self.cursor;
            }
            window.notify_text_selection_changed(&self.focus);
        }
        cx.notify();
    }
}

impl EntityInputHandler for PickerInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.utf8_range(range);
        *adjusted = Some(self.utf16_range(range.clone()));
        Some(self.text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16_range(self.selection()),
            reversed: self.cursor < self.anchor,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.utf8_range(range);
        self.anchor = range.start;
        self.cursor = range.end;
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.utf16_offset(self.text.len()))
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|range| self.utf16_range(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text);
        self.unmark_text(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inserted = self.replace(range, text);
        if let Some(selection) = selection {
            let units = self.utf16_range(inserted.clone());
            self.anchor = self.byte_offset((units.start + selection.start).min(units.end));
            self.cursor = self.byte_offset((units.start + selection.end).min(units.end));
        }
        self.marked = (!inserted.is_empty()).then_some(inserted);
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (line, origin) = self.layout.as_ref()?;
        let range = self.utf8_range(range);
        Some(Bounds::from_corners(
            point(origin.x + line.x_for_index(range.start), bounds.top()),
            point(origin.x + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn selection_bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        self.bounds_for_range(range, bounds, window, cx)
            .into_iter()
            .collect()
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let (line, origin) = self.layout.as_ref()?;
        Some(
            self.utf16_offset(
                self.text
                    .floor_char_boundary(line.closest_index_for_x(point.x - origin.x)),
            ),
        )
    }
}

impl Render for PickerInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let input = cx.entity();
        let empty = self.text.is_empty();
        let text = if empty {
            SharedString::from(self.placeholder)
        } else {
            SharedString::from(self.text.clone())
        };
        let cursor = self.cursor;
        let selection = self.selection();
        let marked = self.marked.clone();
        let focus = self.focus.clone();

        div()
            .id(self.label)
            .role(Role::TextInput)
            .aria_label(self.label)
            .aria_value(self.text.clone())
            .aria_placeholder(self.placeholder)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_click(cx.listener(Self::click))
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .items_center()
            .h(px(44.))
            .w_full()
            .min_w_0()
            .px_3()
            .rounded_md()
            .border_1()
            .border_color(rgb(PICKER_BORDER))
            .focus(|style| style.border_color(rgb(PICKER_FOCUS)))
            .bg(rgb(PICKER_SURFACE))
            .text_color(rgb(PICKER_TEXT))
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, window, _| {
                        let style = window.text_style();
                        let run = TextRun {
                            len: text.len(),
                            font: style.font(),
                            color: if empty {
                                rgb(PICKER_MUTED).into()
                            } else {
                                style.color
                            },
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let font_size = style.font_size.to_pixels(window.rem_size());
                        let line = window
                            .text_system()
                            .shape_line(text, font_size, &[run], None);
                        let offset =
                            (line.x_for_index(cursor) - bounds.size.width + px(2.)).max(px(0.));
                        (line, point(bounds.left() - offset, bounds.top()))
                    },
                    move |bounds, (line, origin), window, cx| {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, input.clone()),
                            cx,
                        );
                        let x = |index| origin.x + line.x_for_index(index);
                        if focus.is_focused(window) {
                            if !selection.is_empty() {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(x(selection.start), bounds.top()),
                                        size(
                                            x(selection.end) - x(selection.start),
                                            bounds.size.height,
                                        ),
                                    ),
                                    rgb(PICKER_BORDER),
                                ));
                            }
                            if let Some(marked) = marked {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(x(marked.start), bounds.bottom() - px(1.)),
                                        size(x(marked.end) - x(marked.start), px(1.)),
                                    ),
                                    rgb(PICKER_FOCUS),
                                ));
                            }
                            if selection.is_empty() {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        point(x(cursor), bounds.top()),
                                        size(px(1.), bounds.size.height),
                                    ),
                                    rgb(PICKER_FOCUS),
                                ));
                            }
                        }
                        if let Err(error) = line.paint(
                            origin,
                            bounds.size.height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        ) {
                            log::debug!("cannot paint style picker input: {error:#}");
                        }
                        input.update(cx, |input, _| input.layout = Some((line, origin)));
                    },
                )
                .w_full()
                .h(px(22.)),
            )
    }
}
