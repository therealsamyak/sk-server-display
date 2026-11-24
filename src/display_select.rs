//! Status display selection: which GPUI display shows the status window.

use std::rc::Rc;

use crate::status_display::DisplaySelector;
use gpui::{App, Bounds, DisplayId, Pixels, PlatformDisplay};

/// The display chosen for the status window, with a label for the controls.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayChoice {
    pub id: DisplayId,
    pub bounds: Bounds<Pixels>,
    pub label: String,
}

/// Returns all displays in left-to-right, top-to-bottom order.
pub fn ordered_status_displays(cx: &App) -> Vec<Rc<dyn PlatformDisplay>> {
    let mut displays = cx.displays();
    displays.sort_by(|a, b| {
        let (a, b) = (a.bounds().origin, b.bounds().origin);
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });
    displays
}

/// Chooses the status display for a selector. Returns `None` with no displays.
pub fn choose_status_display(selector: DisplaySelector, cx: &App) -> Option<DisplayChoice> {
    let displays = ordered_status_displays(cx);
    let number_of = |id: DisplayId| displays.iter().position(|d| d.id() == id).map(|i| i + 1);
    let choice = |display: &Rc<dyn PlatformDisplay>, detail: &str| {
        let number = number_of(display.id()).unwrap_or(0);
        DisplayChoice {
            id: display.id(),
            bounds: display.bounds(),
            label: format!("Display {number} of {} ({detail})", displays.len()),
        }
    };

    match selector {
        DisplaySelector::Number(number) => {
            let display = displays.get(number - 1).or_else(|| displays.first())?;
            Some(choice(display, "selected"))
        }
        DisplaySelector::Primary => {
            primary_or_first(cx, &displays).map(|(d, detail)| choice(&d, detail))
        }
        DisplaySelector::Auto => {
            #[cfg(target_os = "windows")]
            {
                use crate::display_windows::{
                    find_physical_windows_display, list_windows_displays,
                };
                let windows_displays = list_windows_displays();
                if let Some(physical) = find_physical_windows_display(&windows_displays)
                    && let Some(display) = displays
                        .iter()
                        .find(|d| d.id() == DisplayId::new(physical.monitor_handle as u64))
                {
                    return Some(choice(display, &physical.windows_display_description()));
                }
            }
            primary_or_first(cx, &displays).map(|(d, detail)| choice(&d, detail))
        }
    }
}

/// Returns the primary display, or the first display when the platform
/// reports no primary display. Wayland in GPUI reports none.
fn primary_or_first(
    cx: &App,
    displays: &[Rc<dyn PlatformDisplay>],
) -> Option<(Rc<dyn PlatformDisplay>, &'static str)> {
    cx.primary_display()
        .and_then(|primary| displays.iter().find(|d| d.id() == primary.id()).cloned())
        .map(|display| (display, "primary"))
        .or_else(|| {
            displays
                .first()
                .cloned()
                .map(|display| (display, "first display"))
        })
}
