//! Status display fonts: the bundled IBM Plex Sans font family.

use std::borrow::Cow;

use gpui::App;

/// Font family of all status display text. The app bundles it, so text looks
/// the same on Windows, macOS, and Linux.
pub const STATUS_FONT_FAMILY: &str = "IBM Plex Sans";

/// Registers the bundled status display fonts with the GPUI text system.
pub fn load_status_fonts(cx: &App) {
    let fonts = vec![
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf").as_slice(),
        ),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load status display fonts: {error:#}");
    }
}
