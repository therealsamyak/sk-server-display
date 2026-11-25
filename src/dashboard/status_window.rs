//! Status window controller: opens the status window on the chosen display
//! and reopens it when monitors change.

use std::time::Duration;

use gpui::{
    App, AppContext as _, Bounds, Context, DisplayEvent, Entity, Subscription, Task,
    TitlebarOptions, WindowBounds, WindowHandle, WindowOptions, size,
};
use time::UtcOffset;

use crate::platform::DisplaySelector;

use crate::{
    dashboard::status_view::{StatusView, StatusViewSettings},
    metrics::StatusBoard,
    platform::{DisplayChoice, choose_status_display, ordered_status_displays},
    wallpaper::StatusCarousel,
};

/// Wait time after a display event before the window moves. Windows and
/// macOS report one monitor change as several events.
const DISPLAY_CHANGE_SETTLE_DELAY: Duration = Duration::from_millis(750);

/// Startup settings for the status window.
#[derive(Clone, Debug)]
pub struct StatusWindowSettings {
    pub selector: DisplaySelector,
    pub start_fullscreen: bool,
    pub local_offset: UtcOffset,
}

/// Status display app: owns the status window and its display placement.
pub struct StatusDisplayApp {
    board: Entity<StatusBoard>,
    carousel: Entity<StatusCarousel>,
    settings: StatusWindowSettings,
    selector: DisplaySelector,
    fullscreen: bool,
    window: Option<WindowHandle<StatusView>>,
    display: Option<DisplayChoice>,
    reconcile_task: Option<Task<()>>,
    _display_subscription: Subscription,
}

impl StatusDisplayApp {
    /// Starts the status display: chooses a display and opens the window.
    pub fn start_status_display(
        board: Entity<StatusBoard>,
        carousel: Entity<StatusCarousel>,
        settings: StatusWindowSettings,
        cx: &mut App,
    ) -> Entity<Self> {
        let app = cx.new(|cx| {
            let this = cx.weak_entity();
            let display_subscription = cx.observe_displays(move |event, cx| {
                log::info!("display event: {event:?}");
                if matches!(
                    event,
                    DisplayEvent::Added(_)
                        | DisplayEvent::Removed(_)
                        | DisplayEvent::Changed { .. }
                ) {
                    this.update(cx, |app: &mut StatusDisplayApp, cx| {
                        app.schedule_display_reconcile(DISPLAY_CHANGE_SETTLE_DELAY, cx)
                    })
                    .ok();
                }
            });
            Self {
                board,
                carousel,
                selector: settings.selector,
                fullscreen: settings.start_fullscreen,
                settings,
                window: None,
                display: None,
                reconcile_task: None,
                _display_subscription: display_subscription,
            }
        });
        app.update(cx, |app, cx| app.reconcile_status_display(cx));
        app
    }

    /// Moves the status window to the next display in left-to-right order.
    pub fn cycle_status_display(&mut self, cx: &mut Context<Self>) {
        let displays = ordered_status_displays(cx);
        if displays.is_empty() {
            return;
        }
        let current = self
            .display
            .as_ref()
            .and_then(|choice| displays.iter().position(|d| d.id() == choice.id))
            .unwrap_or(0);
        self.selector = DisplaySelector::Number((current + 1) % displays.len() + 1);
        self.schedule_display_reconcile(Duration::ZERO, cx);
    }

    fn schedule_display_reconcile(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.reconcile_task = Some(cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            this.update(cx, |app, cx| app.reconcile_status_display(cx))
                .ok();
        }));
    }

    /// Opens or reopens the status window when the chosen display changed.
    fn reconcile_status_display(&mut self, cx: &mut Context<Self>) {
        let Some(choice) = choose_status_display(self.selector, cx) else {
            log::warn!("no display available for the status window");
            return;
        };
        let window_is_open = self
            .window
            .is_some_and(|window| window.update(cx, |_, _, _| ()).is_ok());
        if window_is_open && self.display.as_ref() == Some(&choice) {
            return;
        }
        log::info!("opening status window on {}", choice.label);

        if let Some(old) = self.window.take()
            && let Ok(fullscreen) = old.update(cx, |_, window, _| window.is_fullscreen())
        {
            self.fullscreen = fullscreen;
            self.open_status_window(&choice, cx);
            old.update(cx, |_, window, _| window.remove_window()).ok();
        } else {
            self.open_status_window(&choice, cx);
        }
        self.display = Some(choice);
    }

    fn open_status_window(&mut self, choice: &DisplayChoice, cx: &mut Context<Self>) {
        let windowed = Bounds::centered(
            Some(choice.id),
            size(
                choice.bounds.size.width * 0.8,
                choice.bounds.size.height * 0.8,
            ),
            cx,
        );
        let options = WindowOptions {
            window_bounds: Some(if self.fullscreen {
                WindowBounds::Fullscreen(windowed)
            } else {
                WindowBounds::Windowed(windowed)
            }),
            display_id: Some(choice.id),
            titlebar: Some(TitlebarOptions {
                title: Some("Server status display".into()),
                ..Default::default()
            }),
            focus: true,
            show: true,
            app_id: Some("server-status-display".into()),
            ..Default::default()
        };
        let board = self.board.clone();
        let controller = cx.weak_entity();
        let label = choice.label.clone();
        let local_offset = self.settings.local_offset;
        let carousel = self.carousel.clone();
        match cx.open_window(options, move |window, cx| {
            cx.new(|cx| {
                StatusView::new_status_view(
                    StatusViewSettings {
                        display_label: label,
                        local_offset,
                    },
                    board,
                    carousel,
                    controller,
                    window,
                    cx,
                )
            })
        }) {
            Ok(window) => self.window = Some(window),
            Err(error) => log::error!("failed to open status window: {error:#}"),
        }
    }
}
