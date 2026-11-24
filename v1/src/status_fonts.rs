//! Status display fonts: the bundled IBM Plex Sans and IBM Plex Mono families.

use std::borrow::Cow;

use gpui::App;

/// Font family of the clock and the date. The app bundles it, so text looks
/// the same on Windows, macOS, and Linux.
pub const STATUS_CLOCK_FONT_FAMILY: &str = "IBM Plex Sans";

/// Monospace font family of the status panels, so digits keep their columns.
pub const STATUS_PANEL_FONT_FAMILY: &str = "IBM Plex Mono";

/// Registers the bundled status display fonts with the GPUI text system.
pub fn load_status_fonts(cx: &App) {
    let fonts = vec![
        Cow::Borrowed(
            include_bytes!("../../assets/fonts/ibm-plex-sans/IBMPlexSans-Light.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../../assets/fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../../assets/fonts/ibm-plex-mono/IBMPlexMono-SemiBold.ttf").as_slice(),
        ),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load status display fonts: {error:#}");
    }
}
