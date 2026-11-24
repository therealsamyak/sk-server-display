//! Windows window helper: stops the GPUI status window from taking keyboard
//! focus away from a streamed game.

use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Adds `WS_EX_NOACTIVATE` to the GPUI window, so a click does not make it
/// the foreground window.
pub fn apply_no_activate_style(window: &Window) {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    if let RawWindowHandle::Win32(handle) = handle.as_raw() {
        server_status_core::display_windows::apply_no_activate_style(handle.hwnd.get());
    }
}
