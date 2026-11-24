//! Status display fonts: the bundled IBM Plex Sans, Serif, and Mono families.

use std::borrow::Cow;

use gpui::App;

/// IBM Plex Sans: the clock and the panel titles of most styles. The app
/// bundles each family, so text looks the same on Windows, macOS, and Linux.
pub const STATUS_SANS_FONT_FAMILY: &str = "IBM Plex Sans";

/// IBM Plex Serif: the clock and the panel titles of the serif styles.
pub const STATUS_SERIF_FONT_FAMILY: &str = "IBM Plex Serif";

/// IBM Plex Mono: the panel text, so digits keep their columns.
pub const STATUS_MONO_FONT_FAMILY: &str = "IBM Plex Mono";

/// Registers the bundled status display fonts with the GPUI text system.
pub fn load_status_fonts(cx: &App) {
    let fonts = vec![
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-sans/IBMPlexSans-Light.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-serif/IBMPlexSerif-Light.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-serif/IBMPlexSerif-Regular.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf").as_slice(),
        ),
        Cow::Borrowed(
            include_bytes!("../assets/fonts/ibm-plex-mono/IBMPlexMono-SemiBold.ttf").as_slice(),
        ),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::error!("failed to load status display fonts: {error:#}");
    }
}
