//! Dashboard layout and interactive overlays.

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, FontWeight, Hsla, IntoElement, Role,
    Stateful, StyleRefinement, Window, div, linear_color_stop, linear_gradient, prelude::*, px,
    relative, rgb,
};
use time::{OffsetDateTime, UtcOffset};

use super::{StatusView, WallpaperPreview};
use crate::{
    dashboard::{
        status_format::{format_status_age, format_status_clock, format_status_date},
        status_panel::{FeedPanelView, expanded_panel_height, side_panel_height},
        status_redraw::StatusRedrawTimer,
        status_theme::{HOST_CARD_HEIGHT, PANEL_GAP, Scale, StatusLayout, StatusTheme},
    },
    metrics::{FeedFreshness, StatusCollector, StatusFeed},
    wallpaper::{PanelGroup, StatusStyle, StyleAnchor, StyleArea},
};

/// Alert for a feed that is stale or offline. Live feeds give none.
pub(super) fn feed_alert(
    s: Scale,
    theme: StatusTheme,
    feed: &StatusFeed,
    freshness: FeedFreshness,
    redraw: &StatusRedrawTimer,
) -> Option<Div> {
    let name = format!("{} ({})", feed.spec.title, feed.spec.source);
    match freshness {
        FeedFreshness::Stale => Some(alert_row(
            s,
            theme,
            theme.warn(),
            format!("{name} is not responding."),
            format!(
                "Data is {} old. {}",
                feed.last_success_age()
                    .map(|age| redraw.status_age_text(age))
                    .unwrap_or_default(),
                feed.last_poll_error().unwrap_or_default(),
            ),
        )),
        FeedFreshness::Offline => Some(alert_row(
            s,
            theme,
            theme.bad(),
            format!("{name} is offline."),
            format!(
                "Retrying every {}.",
                format_status_age(feed.policy.poll_interval),
            ),
        )),
        FeedFreshness::Waiting | FeedFreshness::Live => None,
    }
}

pub(super) fn alert_row(
    s: Scale,
    theme: StatusTheme,
    color: Hsla,
    title: String,
    detail: String,
) -> Div {
    div()
        .max_w_full()
        .flex()
        .flex_col()
        .gap(s(2.))
        .px(s(16.))
        .py(s(8.))
        .rounded(s(8.))
        .bg(theme.panel())
        .border_1()
        .border_color(color.opacity(0.7))
        .child(
            div()
                .text_size(s(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(title),
        )
        .child(
            div()
                .text_size(s(13.))
                .text_color(theme.muted())
                .text_ellipsis()
                .line_clamp(2)
                .child(detail),
        )
}

pub(super) fn control_button(
    s: Scale,
    theme: StatusTheme,
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .min_h(px(44.))
        .flex()
        .items_center()
        .px(s(12.))
        .py(s(4.))
        .rounded(s(6.))
        .bg(theme.panel())
        .border_1()
        .border_color(theme.panel_border())
        .text_size(s(13.))
        .text_color(theme.muted())
        .cursor_pointer()
        .hover(move |style| style.text_color(theme.text()))
        .on_click(on_click)
        .child(label)
}

impl StatusView {
    fn feed_card(
        &self,
        index: usize,
        feed: &StatusFeed,
        panel: &Entity<FeedPanelView>,
        layout: StatusLayout,
        cx: &Context<Self>,
    ) -> AnyElement {
        let expanded = self
            .details
            .as_ref()
            .filter(|details| details.index == index);
        let height = expanded.map_or_else(
            || side_panel_height(feed, &self.carded),
            |_| expanded_panel_height(feed, layout),
        );
        let panel = expanded.map_or(panel, |details| &details.panel);
        div()
            .id(("feed-card", index))
            .w_full()
            .h(layout.scale(height))
            .flex_none()
            .when(expanded.is_none(), |card| {
                card.role(Role::Button)
                    .aria_label(format!("Expand {}", feed.spec.title))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.open_feed_details(index, window, cx)
                    }))
            })
            .child(panel.clone().cached(StyleRefinement::default().size_full()))
            .into_any_element()
    }

    pub(super) fn wallpaper_preview_bar(
        &self,
        s: Scale,
        theme: StatusTheme,
        preview: &WallpaperPreview,
        cx: &Context<Self>,
    ) -> AnyElement {
        let ready = self.preview_ready(preview);
        let message = self.preview_error.clone().unwrap_or_else(|| {
            if ready {
                preview.title.clone()
            } else {
                "Loading preview…".into()
            }
        });
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .items_end()
            .pb(s(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(s(12.))
                    .max_w_full()
                    .flex_wrap()
                    .p(s(8.))
                    .rounded(s(8.))
                    .bg(rgb(0x1e2630))
                    .text_color(rgb(0xf3f5f7))
                    .child(
                        div()
                            .px(s(6.))
                            .text_size(s(14.))
                            .max_w(s(520.))
                            .child(message),
                    )
                    .child(
                        control_button(
                            s,
                            theme,
                            "cancel-wallpaper-preview",
                            "Back",
                            cx.listener(|view, _, window, cx| view.cancel_preview(window, cx)),
                        )
                        .min_h(px(44.))
                        .flex()
                        .items_center(),
                    )
                    .when(ready, |bar| {
                        bar.child(
                            control_button(
                                s,
                                theme,
                                "apply-wallpaper-preview",
                                "Use wallpaper",
                                cx.listener(|view, _, window, cx| view.apply_preview(window, cx)),
                            )
                            .min_h(px(44.))
                            .flex()
                            .items_center(),
                        )
                    }),
            )
            .into_any_element()
    }

    /// Band arrangement: the clock and the notices at the top, all panels in
    /// a band at the bottom.
    pub(super) fn band_content(
        &self,
        s: Scale,
        layout: StatusLayout,
        clock: Div,
        notices: Div,
        cx: &Context<Self>,
    ) -> Div {
        let board = self.board.read(cx);
        let top = div()
            .flex_1()
            .min_h_0()
            .flex()
            .justify_between()
            .items_start()
            .gap(s(32.))
            .child(clock)
            .child(notices.items_end());
        let panel_style = || StyleRefinement::default().flex_1().min_w_0().min_h_0();
        // Presence cards match the height of a collapsed metrics card.
        let status_card_style = || {
            StyleRefinement::default()
                .flex_none()
                .min_w_0()
                .h(s(HOST_CARD_HEIGHT))
        };
        let mut host_panels = Vec::new();
        let mut service_panels = Vec::new();
        let mut side_panels = Vec::new();
        for (index, (feed, panel)) in board.status_feeds().iter().zip(&self.panels).enumerate() {
            let style = match feed.spec.collector {
                StatusCollector::TailnetHost(_) => status_card_style(),
                _ => panel_style(),
            };
            let panel = if feed.spec.collector.has_expandable_panel() {
                self.feed_card(index, feed, panel, layout, cx)
            } else {
                panel.clone().cached(style).into_any_element()
            };
            match &feed.spec.collector {
                StatusCollector::Glances(_)
                | StatusCollector::Local(_)
                | StatusCollector::TailnetHost(_) => host_panels.push(panel),
                StatusCollector::Kuma(_) => service_panels.push(panel),
                StatusCollector::Scrutiny(_) | StatusCollector::Tailscale(_) => {
                    side_panels.push(panel)
                }
            }
        }
        let host_count = host_panels.len();
        let column = |panels: Vec<AnyElement>, grow: f32| {
            (!panels.is_empty()).then(|| {
                let mut column = div()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap(s(PANEL_GAP))
                    .children(panels);
                column.style().flex_grow = Some(grow);
                column.style().flex_basis = Some(relative(0.).into());
                column
            })
        };
        let host_columns = if self.host_rows == 1 {
            let grow = if host_count == 1 { 1.3 } else { 1. };
            host_panels
                .into_iter()
                .filter_map(|panel| column(vec![panel], grow))
                .collect()
        } else {
            column(host_panels, 1.3).into_iter().collect::<Vec<_>>()
        };
        let panels = div()
            .h(s(layout.panel_area))
            .flex()
            .gap(s(PANEL_GAP))
            .children(host_columns)
            .children(column(service_panels, 1.))
            .children(column(side_panels, 1.));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .gap(s(24.))
            .p(s(SCREEN_PADDING))
            .child(top)
            .child(panels)
    }

    /// Side arrangement: one column area at each edge, the middle free for
    /// the art. `clock` holds the clock and the notices.
    pub(super) fn sides_content(
        &self,
        s: Scale,
        layout: StatusLayout,
        clock: Div,
        [left, right]: [StyleArea; 2],
        cx: &Context<Self>,
    ) -> Div {
        let board = self.board.read(cx);
        let mut hosts = Vec::new();
        let mut others = Vec::new();
        for (index, (feed, panel)) in board.status_feeds().iter().zip(&self.panels).enumerate() {
            let height = s(side_panel_height(feed, &self.carded));
            let style = StyleRefinement::default().flex_none().w_full().h(height);
            let panel = if feed.spec.collector.has_expandable_panel() {
                self.feed_card(index, feed, panel, layout, cx)
            } else {
                panel.clone().cached(style).into_any_element()
            };
            if feed.spec.collector.is_host_collector() {
                hosts.push(panel);
            } else {
                others.push(panel);
            }
        }
        let mut clock = Some(clock);
        let mut groups = Some((hosts, others));
        let inner_width = layout.width - 2. * SCREEN_PADDING;
        let mut area = |area: StyleArea, is_right: bool| {
            (area.width > 0.).then(|| {
                let area_clock = area
                    .clock
                    .and_then(|anchor| clock.take().map(|c| (anchor, c)));
                let (hosts, others) = groups.take().unwrap_or_default();
                let (mut hosts, mut others) = (Some(hosts), Some(others));
                let mut columns: Vec<SideColumn> = area
                    .groups
                    .iter()
                    .map(|group| {
                        let panels = match group {
                            PanelGroup::Hosts => hosts.take(),
                            PanelGroup::Others => others.take(),
                        };
                        let grow = match group {
                            PanelGroup::Hosts => 1.,
                            PanelGroup::Others => OTHERS_COLUMN_GROW,
                        };
                        (None, panels.unwrap_or_default(), grow)
                    })
                    .collect();
                // Panels not placed in this area go to the other area.
                groups = Some((hosts.unwrap_or_default(), others.unwrap_or_default()));
                // The clock stands on the host column, or alone.
                let clock_column = area
                    .groups
                    .iter()
                    .position(|group| *group == PanelGroup::Hosts)
                    .unwrap_or(0);
                match columns.get_mut(clock_column) {
                    Some(column) => column.0 = area_clock,
                    None => columns.push((area_clock, Vec::new(), 1.)),
                }
                if is_right {
                    columns.reverse();
                }
                div()
                    .flex_none()
                    .w(s(area.width * inner_width))
                    .h_full()
                    .flex()
                    .gap(s(PANEL_GAP))
                    .children(columns.into_iter().map(|(clock, panels, grow)| {
                        side_column(s, clock, panels, area.panels, grow)
                    }))
            })
        };
        // The left area takes its groups first. The right area gets the rest.
        let left_area = area(left, false);
        let right_area = area(right, true);
        div()
            .relative()
            .size_full()
            .flex()
            .gap(s(PANEL_GAP))
            .p(s(SCREEN_PADDING))
            .children(left_area)
            .child(div().flex_1())
            .children(right_area)
    }
}

/// One column of a side area: the clock block with its anchor, if any, and
/// the panels.
type SideColumn = (Option<(StyleAnchor, Div)>, Vec<AnyElement>, f32);

/// Width share of the services, disks, and tailnet column next to a host
/// column. Its rows have more columns than the host cards.
const OTHERS_COLUMN_GROW: f32 = 1.15;

/// Padding at the window edges, in design pixels.
pub(super) const SCREEN_PADDING: f32 = 32.;

/// Width of the shade fade beyond a side area, in design pixels.
const SHADE_FADE: f32 = 200.;

/// One column of a side area: the clock block, if any, and a panel stack.
fn side_column(
    s: Scale,
    clock: Option<(StyleAnchor, Div)>,
    panels: Vec<AnyElement>,
    anchor: StyleAnchor,
    grow: f32,
) -> Div {
    let stack = (!panels.is_empty()).then(|| {
        div()
            .w_full()
            .min_h_0()
            .flex_shrink(1.)
            .flex()
            .flex_col()
            .gap(s(PANEL_GAP))
            .children(panels)
    });
    let mut column = div().min_w_0().h_full().flex().flex_col().gap(s(24.));
    column.style().flex_grow = Some(grow);
    column.style().flex_basis = Some(relative(0.).into());
    match (clock, stack) {
        (Some((clock_anchor, clock)), Some(stack)) => {
            let clock = clock.flex_none();
            // A free space between the two blocks puts each at its own edge.
            let spacer = (clock_anchor != anchor).then(|| div().flex_1());
            let column = column.when(
                clock_anchor == StyleAnchor::Bottom && anchor == StyleAnchor::Bottom,
                |column| column.justify_end(),
            );
            match clock_anchor {
                StyleAnchor::Top => column.child(clock).children(spacer).child(stack),
                StyleAnchor::Bottom => column.child(stack).children(spacer).child(clock),
            }
        }
        (Some((clock_anchor, clock)), None) => column
            .when(clock_anchor == StyleAnchor::Bottom, |column| {
                column.justify_end()
            })
            .child(clock),
        (None, Some(stack)) => column
            .when(anchor == StyleAnchor::Bottom, |column| column.justify_end())
            .child(stack),
        (None, None) => column,
    }
}

/// Shade behind one side area. It fades out toward the middle of the screen.
pub(super) fn area_shade(
    s: Scale,
    theme: StatusTheme,
    layout: StatusLayout,
    area: StyleArea,
    is_right: bool,
) -> Div {
    let solid = SCREEN_PADDING + area.width * (layout.width - 2. * SCREEN_PADDING);
    let total = solid + SHADE_FADE;
    let shade = theme.shade();
    let gradient = linear_gradient(
        if is_right { 270. } else { 90. },
        linear_color_stop(shade, solid / total),
        linear_color_stop(shade.opacity(0.), 1.),
    );
    let strip = div().absolute().top_0().bottom_0().w(s(total)).bg(gradient);
    if is_right {
        strip.right_0()
    } else {
        strip.left_0()
    }
}

/// Clock and date. `at_end` aligns them to the right edge.
pub(super) fn status_clock(
    s: Scale,
    style: StatusStyle,
    now: OffsetDateTime,
    offset: UtcOffset,
    at_end: bool,
) -> Div {
    let kind = style.kind;
    let theme = StatusTheme(style.palette);
    div()
        .flex()
        .flex_col()
        .when(at_end, |clock| clock.items_end())
        .gap(s(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .when(at_end, |clock| clock.items_end())
                .font_family(kind.clock_family)
                .child(
                    div()
                        .text_size(s(kind.clock_size))
                        .line_height(relative(1.))
                        .whitespace_nowrap()
                        .when(kind.clock_light, |clock| {
                            clock.font_weight(FontWeight::LIGHT)
                        })
                        .text_color(theme.clock())
                        .child(format_status_clock(now, offset)),
                )
                .child(
                    div()
                        .px(s(4.))
                        .whitespace_nowrap()
                        .text_size(s(28.))
                        .text_color(theme.date())
                        .child(format_status_date(now, offset)),
                ),
        )
}
