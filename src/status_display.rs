//! Status display choice: the display selector and the physical display
//! ranking that keeps the status window off virtual displays.

use std::str::FromStr;

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

/// Kind of display output, as Windows display configuration reports it.
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

impl DisplayOutputKind {
    /// Display label text, such as `built-in panel`.
    pub fn display_kind_label(self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in panel",
            Self::External => "external monitor",
            Self::IndirectWired => "USB display",
            Self::Virtual => "virtual display",
        }
    }
}

/// One display candidate for the physical status display ranking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalDisplayCandidate {
    pub kind: DisplayOutputKind,
    pub is_primary: bool,
}

/// Ranks candidates for the physical status display and returns the best index.
///
/// Order: built-in panel, primary external, other external, indirect wired.
/// A virtual display is never chosen.
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
