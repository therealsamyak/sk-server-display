//! Display placement: which display shows the status window, and moving the
//! window when the selector or the monitors change.
//!
//! Rune reports the displays once, at startup. On Windows the placement
//! watcher reads the live display configuration every 2 seconds, so the
//! window follows monitor changes. On macOS and Linux the startup list is
//! the only list: a monitor change needs a restart.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    time::Duration,
};

use rune::AppHandle;
use server_status_core::status_display::DisplaySelector;

use crate::status_state::StatusState;

/// Time between two reads of the Windows display configuration.
#[cfg(target_os = "windows")]
const DISPLAY_WATCH_INTERVAL: Duration = Duration::from_secs(2);

/// Wait between the steps of a fullscreen window move, so the platform
/// finishes leaving fullscreen before the window moves.
const WINDOW_MOVE_STEP_DELAY: Duration = Duration::from_millis(300);

/// One display that can show the status window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayOption {
    /// Top-left corner in physical desktop pixels.
    pub position: (i32, i32),
    /// Size in physical pixels.
    pub size: (u32, u32),
    /// Platform name. On Windows this is the GDI device name.
    pub name: Option<String>,
    pub is_primary: bool,
}

/// The display chosen for the status window, with its label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayChoice {
    pub display: DisplayOption,
    /// 1-based number in left-to-right, top-to-bottom order.
    pub number: usize,
    /// Label for the window controls, such as `Display 1 of 2 (primary)`.
    pub label: String,
}

/// Converts Rune's startup display list to display options in
/// left-to-right, top-to-bottom order.
pub fn rune_display_options(displays: &[rune::Display]) -> Vec<DisplayOption> {
    let mut options: Vec<DisplayOption> = displays
        .iter()
        .map(|display| DisplayOption {
            position: display.position,
            size: display.size,
            name: display.name.clone(),
            is_primary: display.primary,
        })
        .collect();
    options.sort_by_key(|option| option.position);
    options
}

/// Chooses the status display for a selector. Returns `None` with no displays.
pub fn choose_display_option(
    selector: DisplaySelector,
    options: &[DisplayOption],
) -> Option<DisplayChoice> {
    let choice = |index: usize, detail: &str| DisplayChoice {
        display: options[index].clone(),
        number: index + 1,
        label: format!("Display {} of {} ({detail})", index + 1, options.len()),
    };
    let primary_or_first = || match options.iter().position(|o| o.is_primary) {
        Some(index) => Some(choice(index, "primary")),
        None => (!options.is_empty()).then(|| choice(0, "first display")),
    };
    match selector {
        DisplaySelector::Number(number) => {
            let index = if number <= options.len() {
                number - 1
            } else {
                0
            };
            (!options.is_empty()).then(|| choice(index, "selected"))
        }
        DisplaySelector::Primary => primary_or_first(),
        DisplaySelector::Auto => {
            physical_display_choice(options, &choice).or_else(primary_or_first)
        }
    }
}

#[cfg(target_os = "windows")]
fn physical_display_choice(
    options: &[DisplayOption],
    choice: &dyn Fn(usize, &str) -> DisplayChoice,
) -> Option<DisplayChoice> {
    use server_status_core::display_windows::{
        find_physical_windows_display, list_windows_displays,
    };
    let windows_displays = list_windows_displays();
    let physical = find_physical_windows_display(&windows_displays)?;
    let index = options.iter().position(|option| {
        option.name.as_deref() == Some(physical.gdi_device_name.as_str())
            || option.position == physical.position
    })?;
    Some(choice(index, &physical.windows_display_description()))
}

#[cfg(not(target_os = "windows"))]
fn physical_display_choice(
    _options: &[DisplayOption],
    _choice: &dyn Fn(usize, &str) -> DisplayChoice,
) -> Option<DisplayChoice> {
    None
}

/// The current display options. Windows reads the live configuration;
/// other platforms keep Rune's startup list.
fn current_display_options(startup: &[DisplayOption]) -> Vec<DisplayOption> {
    #[cfg(target_os = "windows")]
    {
        let mut options: Vec<DisplayOption> =
            server_status_core::display_windows::list_windows_displays()
                .into_iter()
                .map(|display| DisplayOption {
                    position: display.position,
                    size: display.size,
                    name: Some(display.gdi_device_name),
                    is_primary: display.is_primary,
                })
                .collect();
        options.sort_by_key(|option| option.position);
        if !options.is_empty() {
            return options;
        }
    }
    startup.to_vec()
}

/// Shared placement state: the selector, the fullscreen flag, and the
/// watcher's wake-up channel.
pub struct DisplayPlacement {
    selector: Mutex<DisplaySelector>,
    current: Mutex<Option<DisplayChoice>>,
    full_screen: AtomicBool,
    nudge: Sender<()>,
}

impl DisplayPlacement {
    /// Creates the placement state for the first display choice.
    pub fn new_display_placement(
        selector: DisplaySelector,
        first: Option<DisplayChoice>,
        full_screen: bool,
    ) -> (Arc<Self>, Receiver<()>) {
        let (nudge, receiver) = mpsc::channel();
        let placement = Arc::new(Self {
            selector: Mutex::new(selector),
            current: Mutex::new(first),
            full_screen: AtomicBool::new(full_screen),
            nudge,
        });
        (placement, receiver)
    }

    /// `true` while the window is fullscreen, as this app last set it.
    pub fn is_full_screen(&self) -> bool {
        self.full_screen.load(Ordering::Relaxed)
    }

    /// Records a fullscreen change that the UI thread made.
    pub fn set_full_screen(&self, full_screen: bool) {
        self.full_screen.store(full_screen, Ordering::Relaxed);
    }

    /// Selects the next display in left-to-right order and moves the window.
    pub fn cycle_status_display(&self, display_count: usize) {
        let current = self
            .current
            .lock()
            .ok()
            .and_then(|current| current.as_ref().map(|c| c.number))
            .unwrap_or(0);
        if let Ok(mut selector) = self.selector.lock() {
            *selector = DisplaySelector::Number(current % display_count.max(1) + 1);
        }
        self.nudge.send(()).ok();
    }
}

/// Starts the placement watcher thread. It moves the window when the
/// chosen display changes.
pub fn start_placement_watcher(
    handle: AppHandle<StatusState>,
    placement: Arc<DisplayPlacement>,
    startup: Vec<DisplayOption>,
    nudges: Receiver<()>,
) {
    let spawned = std::thread::Builder::new()
        .name("status-placement".into())
        .spawn(move || run_placement_watcher(handle, placement, startup, nudges));
    if let Err(error) = spawned {
        log::error!("cannot start the display placement watcher: {error}");
    }
}

fn run_placement_watcher(
    handle: AppHandle<StatusState>,
    placement: Arc<DisplayPlacement>,
    startup: Vec<DisplayOption>,
    nudges: Receiver<()>,
) {
    #[cfg(target_os = "windows")]
    let wait = DISPLAY_WATCH_INTERVAL;
    #[cfg(not(target_os = "windows"))]
    let wait = Duration::MAX;
    loop {
        match nudges.recv_timeout(wait) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let options = current_display_options(&startup);
        let selector = placement
            .selector
            .lock()
            .map(|s| *s)
            .unwrap_or(DisplaySelector::Auto);
        let Some(choice) = choose_display_option(selector, &options) else {
            continue;
        };
        let current = placement.current.lock().ok().and_then(|c| c.clone());
        if current.as_ref() == Some(&choice) {
            continue;
        }
        let bounds = |c: &DisplayChoice| (c.display.position, c.display.size);
        if current.as_ref().map(bounds) != Some(bounds(&choice)) {
            log::info!("moving the status window to {}", choice.label);
            move_status_window(&handle, &placement, &choice);
        }
        if let Ok(mut current) = placement.current.lock() {
            *current = Some(choice.clone());
        }
        let label = choice.label.clone();
        let count = options.len();
        handle.update(move |state, _| state.set_display_label(label, count));
    }
}

/// Moves the window to the chosen display. A fullscreen window leaves
/// fullscreen, moves, and enters fullscreen again on the new display.
fn move_status_window(
    handle: &AppHandle<StatusState>,
    placement: &DisplayPlacement,
    choice: &DisplayChoice,
) {
    let full_screen = placement.is_full_screen();
    if full_screen {
        handle.update(|_, tx| tx.toggle_full_screen());
        std::thread::sleep(WINDOW_MOVE_STEP_DELAY);
    }
    handle.set_position(windowed_position(&choice.display));
    if full_screen {
        std::thread::sleep(WINDOW_MOVE_STEP_DELAY);
        handle.update(|_, tx| tx.toggle_full_screen());
    }
}

/// Window position on a display: 10 % in from its top-left corner, so the
/// window, at 80 % of the display size, sits in the middle.
pub fn windowed_position(display: &DisplayOption) -> (i32, i32) {
    (
        display.position.0 + (display.size.0 / 10) as i32,
        display.position.1 + (display.size.1 / 10) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(x: i32, is_primary: bool) -> DisplayOption {
        DisplayOption {
            position: (x, 0),
            size: (1920, 1080),
            name: None,
            is_primary,
        }
    }

    #[test]
    fn primary_selector_picks_the_primary_display() {
        let options = [option(0, false), option(1920, true)];
        let choice = choose_display_option(DisplaySelector::Primary, &options).unwrap();
        assert_eq!(choice.number, 2);
        assert_eq!(choice.label, "Display 2 of 2 (primary)");
    }

    #[test]
    fn number_selector_falls_back_to_the_first_display() {
        let options = [option(0, true), option(1920, false)];
        let choice = choose_display_option(DisplaySelector::Number(5), &options).unwrap();
        assert_eq!(choice.number, 1);
    }

    #[test]
    fn windowed_position_is_inside_the_display() {
        assert_eq!(windowed_position(&option(1920, false)), (2112, 108));
    }
}
