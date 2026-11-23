//! Status display selection: which monitor shows the status window.

use std::{rc::Rc, str::FromStr};

use gpui::{App, Bounds, DisplayId, Pixels, PlatformDisplay};

/// Display selector from the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplaySelector {
    /// Windows: the physical built-in or external panel, never a virtual
    /// display. Other platforms: the primary display.
    Auto,
    /// The display that the operating system reports as primary.
    Primary,
    /// A 1-based display number in left-to-right, top-to-bottom order.
    Number(usize),
}

impl FromStr for DisplaySelector {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "primary" => Ok(Self::Primary),
            number => match number.parse::<usize>() {
                Ok(number) if number >= 1 => Ok(Self::Number(number)),
                _ => Err(format!(
                    "invalid display selector `{value}`: use auto, primary, or a number from 1"
                )),
            },
        }
    }
}

/// The display chosen for the status window, with a label for the footer.
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
        let bounds = display.bounds();
        let number = number_of(display.id()).unwrap_or(0);
        DisplayChoice {
            id: display.id(),
            bounds,
            label: format!(
                "Display {number} of {} · {detail} · {:.0}×{:.0}",
                displays.len(),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height)
            ),
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
            if let Some(physical) = crate::display_windows::find_physical_status_display()
                && let Some(display) = displays.iter().find(|d| d.id() == physical.display_id)
            {
                return Some(choice(display, &physical.description));
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

/// Kind of display output, as Windows display configuration reports it.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayOutputKind {
    /// Built-in laptop panel (internal, embedded DisplayPort, or embedded UDI).
    BuiltIn,
    /// External monitor on a physical connector, such as HDMI or DisplayPort.
    External,
    /// Indirect wired display, such as a USB display adapter.
    IndirectWired,
    /// Virtual or unknown output, such as an indirect display driver.
    Virtual,
}

/// One display candidate for the physical status display ranking.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalDisplayCandidate {
    pub kind: DisplayOutputKind,
    pub is_primary: bool,
}

/// Ranks candidates for the physical status display and returns the best index.
///
/// Order: built-in panel, primary external, other external, indirect wired.
/// A virtual display is never chosen.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn rank_physical_display_candidates(candidates: &[PhysicalDisplayCandidate]) -> Option<usize> {
    let score = |candidate: &PhysicalDisplayCandidate| -> Option<u8> {
        let kind = match candidate.kind {
            DisplayOutputKind::BuiltIn => 30,
            DisplayOutputKind::External => 20,
            DisplayOutputKind::IndirectWired => 10,
            DisplayOutputKind::Virtual => return None,
        };
        Some(kind + u8::from(candidate.is_primary))
    };
    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| score(candidate).map(|score| (index, score)))
        .max_by_key(|(index, score)| (*score, std::cmp::Reverse(*index)))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(kind: DisplayOutputKind, is_primary: bool) -> PhysicalDisplayCandidate {
        PhysicalDisplayCandidate { kind, is_primary }
    }

    #[test]
    fn parses_display_selectors() {
        assert_eq!("auto".parse(), Ok(DisplaySelector::Auto));
        assert_eq!("primary".parse(), Ok(DisplaySelector::Primary));
        assert_eq!("2".parse(), Ok(DisplaySelector::Number(2)));
        assert!("0".parse::<DisplaySelector>().is_err());
        assert!("left".parse::<DisplaySelector>().is_err());
    }

    #[test]
    fn built_in_panel_wins_over_primary_virtual_display() {
        let candidates = [
            candidate(DisplayOutputKind::Virtual, true),
            candidate(DisplayOutputKind::BuiltIn, false),
        ];
        assert_eq!(rank_physical_display_candidates(&candidates), Some(1));
    }

    #[test]
    fn primary_external_wins_without_built_in_panel() {
        let candidates = [
            candidate(DisplayOutputKind::External, false),
            candidate(DisplayOutputKind::External, true),
            candidate(DisplayOutputKind::IndirectWired, true),
        ];
        assert_eq!(rank_physical_display_candidates(&candidates), Some(1));
    }

    #[test]
    fn only_virtual_displays_give_no_choice() {
        let candidates = [candidate(DisplayOutputKind::Virtual, true)];
        assert_eq!(rank_physical_display_candidates(&candidates), None);
    }
}
