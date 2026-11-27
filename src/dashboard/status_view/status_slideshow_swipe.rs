//! Horizontal slideshow gestures and timed transitions.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AppContext as _, Context, Div, Entity, ObjectFit, Pixels, Point, RenderImage, StyledImage as _,
    Window, div, img, prelude::*, px,
};
use time::OffsetDateTime;

use super::{StatusView, status_view_layout::PagePanels};
use crate::{
    dashboard::{
        status_panel::FeedPanelView,
        status_theme::{StatusLayout, StatusTheme},
    },
    wallpaper::WallpaperId,
};

const SWIPE_SLOP: Pixels = px(12.);
const SWIPE_SETTLE_TIME: Duration = Duration::from_millis(220);

/// A pending drag or a slide moving horizontally.
pub(super) enum SlideshowSwipe {
    Pressed(Point<Pixels>),
    Dragging {
        origin: Point<Pixels>,
        offset: Pixels,
    },
    Settling {
        from: Pixels,
        to: Pixels,
        neighbor: WallpaperId,
        forward: bool,
        automatic: bool,
        start: Instant,
    },
}

/// Live feed views for the preset coming into view, kept separate from the shown widgets.
pub(super) struct IncomingPanels {
    id: Option<WallpaperId>,
    panels: Vec<Entity<FeedPanelView>>,
}

impl StatusView {
    /// Whether a slide is moving horizontally.
    pub(super) fn swipe_in_motion(&self) -> bool {
        matches!(
            self.swipe,
            Some(SlideshowSwipe::Dragging { .. } | SlideshowSwipe::Settling { .. })
        )
    }

    /// Whether the timer started the current transition.
    pub(super) fn automatic_slide_in_motion(&self) -> bool {
        matches!(
            self.swipe,
            Some(SlideshowSwipe::Settling {
                automatic: true,
                ..
            })
        )
    }

    /// Advance after the slide interval once the next wallpaper is ready.
    pub(super) fn update_slideshow(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.picker.is_some()
            || self.details.is_some()
            || self.btop.is_some()
            || self.update_dialog.is_some()
            || self.pointer_pressed
            || self.swipe_in_motion()
        {
            return;
        }
        let carousel = self.carousel.read(cx);
        let Some(list) = carousel.playing_playlist() else {
            return;
        };
        let interval = list.slide_duration();
        let wait = interval.saturating_sub(carousel.last_switch.elapsed());
        if !wait.is_zero() {
            self.redraw.redraw_after(wait);
            return;
        }
        if carousel.current != self.wallpaper.shown {
            return;
        }
        let Some(neighbor) = carousel
            .adjacent_slide(true)
            .filter(|id| *id != carousel.current)
        else {
            self.carousel
                .update(cx, |carousel, _| carousel.last_switch = Instant::now());
            self.redraw.redraw_after(interval);
            return;
        };
        if !self.slideshow_image_ready(neighbor, window) {
            return;
        }
        self.swipe = Some(SlideshowSwipe::Settling {
            from: px(0.),
            to: -window.viewport_size().width,
            neighbor,
            forward: true,
            automatic: true,
            start: Instant::now(),
        });
        window.request_animation_frame();
    }

    /// Track a press while a slideshow wallpaper can take gestures.
    pub(super) fn begin_swipe(&mut self, position: Point<Pixels>, cx: &Context<Self>) {
        self.swipe_consumed = false;
        if !matches!(self.swipe, Some(SlideshowSwipe::Settling { .. }))
            && self.picker.is_none()
            && self.update_dialog.is_none()
            && self.preview.is_none()
            && self.btop.is_none()
            && self.carousel.read(cx).playing_playlist().is_some()
        {
            self.swipe = Some(SlideshowSwipe::Pressed(position));
        }
    }

    /// Follow horizontal movement after it exceeds the tap tolerance.
    pub(super) fn drag_swipe(
        &mut self,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let origin = match self.swipe {
            Some(SlideshowSwipe::Pressed(origin) | SlideshowSwipe::Dragging { origin, .. }) => {
                origin
            }
            _ => return,
        };
        let delta = position - origin;
        let horizontal = px(f32::from(delta.x).abs());
        if matches!(self.swipe, Some(SlideshowSwipe::Pressed(_))) {
            let vertical = px(f32::from(delta.y).abs());
            if horizontal.max(vertical) < SWIPE_SLOP {
                return;
            }
            if vertical >= horizontal {
                self.swipe = None;
                return;
            }
        }
        let width = window.viewport_size().width;
        self.swipe = Some(SlideshowSwipe::Dragging {
            origin,
            offset: delta.x.clamp(-width, width),
        });
        cx.notify();
    }

    /// Suppress the tap and continue from the released offset to the selected edge.
    pub(super) fn end_swipe(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(SlideshowSwipe::Dragging { offset, .. }) = self.swipe else {
            if matches!(self.swipe, Some(SlideshowSwipe::Pressed(_))) {
                self.swipe = None;
            }
            return;
        };
        self.swipe = None;
        self.swipe_consumed = true;
        cx.notify();

        let width = window.viewport_size().width;
        let forward = offset < px(0.);
        let commit = px(f32::from(offset).abs()) >= (width * 0.2).min(px(120.));
        let Some(neighbor) = self.swipe_neighbor(forward, window, cx) else {
            if commit {
                self.carousel
                    .update(cx, |carousel, _| carousel.step_slideshow(forward));
            }
            return;
        };
        self.swipe = Some(SlideshowSwipe::Settling {
            from: offset,
            to: if !commit {
                px(0.)
            } else if forward {
                -width
            } else {
                width
            },
            neighbor,
            forward,
            automatic: false,
            start: Instant::now(),
        });
    }

    /// Install the destination once its image has covered the window.
    pub(super) fn finish_swipe(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(SlideshowSwipe::Settling {
            to,
            neighbor,
            start,
            ..
        }) = self.swipe
        else {
            return;
        };
        if !self.slideshow_image_ready(neighbor, window) {
            self.swipe = None;
            return;
        }
        if start.elapsed() < SWIPE_SETTLE_TIME {
            window.request_animation_frame();
            return;
        }
        if self.pointer_pressed {
            return;
        }
        self.swipe = None;
        if to != px(0.) {
            if let Some(incoming) = &mut self.incoming_panels
                && incoming.id == Some(neighbor)
            {
                std::mem::swap(&mut self.panels, &mut incoming.panels);
                incoming.id = Some(self.wallpaper.shown);
            }
            self.carousel.update(cx, |carousel, _| {
                carousel.current = neighbor;
                carousel.last_switch = Instant::now();
            });
        }
    }

    fn swipe_neighbor(
        &self,
        forward: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<WallpaperId> {
        let carousel = self.carousel.read(cx);
        if carousel.current != self.wallpaper.shown {
            return None;
        }
        let id = carousel.adjacent_slide(forward)?;
        (id != carousel.current && self.slideshow_image_ready(id, window)).then_some(id)
    }

    fn slideshow_image_ready(&self, id: WallpaperId, window: &Window) -> bool {
        id == WallpaperId::PLAIN || self.wallpaper.warm_image(id, window).is_some()
    }

    /// Prepare the destination colors and views once per neighboring preset.
    pub(super) fn prepare_incoming_panels(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some((_, _, id)) = self.slide_positions(window, cx) else {
            if let Some(details) = &mut self.details {
                details.incoming = None;
            }
            return;
        };
        let style = self.carousel.read(cx).wallpaper_style(id);
        if let Some(details) = &mut self.details
            && details
                .incoming
                .as_ref()
                .is_none_or(|(current, _)| *current != id)
        {
            let panel = details.panel.clone();
            let incoming = cx.new(|cx| FeedPanelView::new_incoming_detail(&panel, style, cx));
            details.incoming = Some((id, incoming));
        }
        let mut incoming = self
            .incoming_panels
            .take()
            .unwrap_or_else(|| IncomingPanels {
                id: None,
                panels: (0..self.panels.len())
                    .map(|index| self.create_feed_panel(index, cx))
                    .collect(),
            });
        if incoming.id != Some(id) {
            incoming.id = Some(id);
            for panel in &incoming.panels {
                panel.update(cx, |panel, cx| panel.set_panel_style(style, cx));
            }
        }
        self.incoming_panels = Some(incoming);
    }

    fn slide_positions(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<(Pixels, bool, WallpaperId)> {
        match self.swipe {
            Some(SlideshowSwipe::Dragging { offset, .. }) => self
                .swipe_neighbor(offset < px(0.), window, cx)
                .map(|id| (offset, offset < px(0.), id)),
            Some(SlideshowSwipe::Settling {
                from,
                to,
                neighbor,
                forward,
                start,
                ..
            }) => {
                let t = (start.elapsed().as_secs_f32() / SWIPE_SETTLE_TIME.as_secs_f32()).min(1.);
                let progress = 1. - (1. - t).powi(3);
                Some((from + (to - from) * progress, forward, neighbor))
            }
            _ => None,
        }
    }

    /// Move each wallpaper, shade, clock, and set of feed cards as one page.
    pub(super) fn slideshow_pages(
        &self,
        window: &Window,
        layout: StatusLayout,
        now: OffsetDateTime,
        cx: &Context<Self>,
    ) -> Div {
        let motion = self.slide_positions(window, cx);
        let width = window.viewport_size().width;
        let page = |left,
                    style: crate::wallpaper::StatusStyle,
                    image: Option<Arc<RenderImage>>,
                    panels: PagePanels<'_>| {
            let theme = StatusTheme(style.palette);
            div()
                .absolute()
                .top_0()
                .left(left)
                .size_full()
                .overflow_hidden()
                .bg(theme.background())
                .children(image.map(|image| {
                    img(image)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                }))
                .child(div().absolute().inset_0().bg(theme.scrim()))
                .child(self.dashboard_content(layout, now, style, panels, cx))
        };
        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .child(
                page(
                    motion.map_or(px(0.), |(offset, _, _)| offset),
                    self.wallpaper.style,
                    self.wallpaper.image.clone(),
                    PagePanels {
                        regular: &self.panels,
                        expanded: self
                            .details
                            .as_ref()
                            .map(|details| (details.index, &details.panel)),
                    },
                )
                .id("active-slideshow-page"),
            )
            .children(motion.zip(self.incoming_panels.as_ref()).map(
                |((offset, forward, id), incoming)| {
                    let side = if forward { width } else { -width };
                    let style = self.carousel.read(cx).wallpaper_style(id);
                    page(
                        offset + side,
                        style,
                        self.wallpaper.warm_image(id, window),
                        PagePanels {
                            regular: &incoming.panels,
                            expanded: self.details.as_ref().and_then(|details| {
                                let (wallpaper, panel) = details.incoming.as_ref()?;
                                (*wallpaper == id).then_some((details.index, panel))
                            }),
                        },
                    )
                    .id("incoming-slideshow-page")
                },
            ))
    }
}
