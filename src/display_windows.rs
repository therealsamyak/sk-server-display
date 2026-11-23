//! Windows display helpers: find the physical status display and stop the
//! status window from taking keyboard focus away from a streamed game.

use std::ffi::c_void;

use gpui::{DisplayId, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::{
    Devices::Display::{
        DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
        DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_OTHER, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED,
        DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DISPLAYCONFIG_TARGET_DEVICE_NAME, DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS,
        QueryDisplayConfig,
    },
    Foundation::{ERROR_SUCCESS, HWND, LPARAM, RECT},
    Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, MONITORINFOF_PRIMARY, SetWindowLongPtrW, WS_EX_NOACTIVATE,
    },
};
use windows::core::BOOL;

use crate::display_select::{
    DisplayOutputKind, PhysicalDisplayCandidate, rank_physical_display_candidates,
};

/// The physical display that Windows display configuration selects.
#[derive(Clone, Debug)]
pub struct PhysicalStatusDisplay {
    /// GPUI display ID. GPUI on Windows uses the `HMONITOR` value as the ID.
    pub display_id: DisplayId,
    /// Footer text, such as `built-in panel "Built-in Display"`.
    pub description: String,
}

struct ActiveDisplayPath {
    gdi_device_name: String,
    friendly_name: String,
    kind: DisplayOutputKind,
}

struct ActiveMonitor {
    handle: HMONITOR,
    gdi_device_name: String,
    is_primary: bool,
}

/// Finds the physical status display: the built-in panel first, then a
/// physical external monitor. Virtual displays are never returned.
pub fn find_physical_status_display() -> Option<PhysicalStatusDisplay> {
    let paths = query_active_display_paths()?;
    let monitors = enumerate_active_monitors();
    let matched: Vec<(&ActiveDisplayPath, &ActiveMonitor)> = paths
        .iter()
        .filter_map(|path| {
            monitors
                .iter()
                .find(|monitor| monitor.gdi_device_name == path.gdi_device_name)
                .map(|monitor| (path, monitor))
        })
        .collect();
    let candidates: Vec<PhysicalDisplayCandidate> = matched
        .iter()
        .map(|(path, monitor)| PhysicalDisplayCandidate {
            kind: path.kind,
            is_primary: monitor.is_primary,
        })
        .collect();
    let (path, monitor) = matched[rank_physical_display_candidates(&candidates)?];
    let kind = match path.kind {
        DisplayOutputKind::BuiltIn => "built-in panel",
        DisplayOutputKind::External => "external monitor",
        DisplayOutputKind::IndirectWired => "USB display",
        DisplayOutputKind::Virtual => "virtual display",
    };
    let name = if path.friendly_name.is_empty() {
        path.gdi_device_name.trim_start_matches(r"\\.\").to_string()
    } else {
        path.friendly_name.clone()
    };
    Some(PhysicalStatusDisplay {
        display_id: DisplayId::new(monitor.handle.0 as u64),
        description: format!("{kind} \"{name}\""),
    })
}

/// Stops the status window from becoming the foreground window on click.
///
/// Sunshine sends remote keyboard input to the foreground window. Without
/// this style, one local click on the status display moves the game's input
/// to the status window.
pub fn apply_no_activate_style(window: &Window) {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(handle.hwnd.get() as *mut c_void);
    // SAFETY: `hwnd` is the live top-level window that GPUI owns.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
    }
}

fn output_kind(technology: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY) -> DisplayOutputKind {
    match technology {
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL
        | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED
        | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED => DisplayOutputKind::BuiltIn,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED => DisplayOutputKind::IndirectWired,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL
        | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_OTHER => DisplayOutputKind::Virtual,
        _ => DisplayOutputKind::External,
    }
}

fn query_active_display_paths() -> Option<Vec<ActiveDisplayPath>> {
    let mut path_count = 0u32;
    let mut mode_count = 0u32;
    // SAFETY: both pointers refer to live `u32` values.
    let status = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
    // SAFETY: the buffers hold the element counts that the call receives.
    let status = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    paths.truncate(path_count as usize);

    let mut result = Vec::new();
    for path in &paths {
        let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
            },
            ..Default::default()
        };
        let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };
        // SAFETY: each header has the type and size of its enclosing packet.
        let (target_status, source_status) = unsafe {
            (
                DisplayConfigGetDeviceInfo(&mut target.header),
                DisplayConfigGetDeviceInfo(&mut source.header),
            )
        };
        if target_status != 0 || source_status != 0 {
            continue;
        }
        result.push(ActiveDisplayPath {
            gdi_device_name: wide_to_string(&source.viewGdiDeviceName),
            friendly_name: wide_to_string(&target.monitorFriendlyDeviceName),
            kind: output_kind(target.outputTechnology),
        });
    }
    Some(result)
}

fn enumerate_active_monitors() -> Vec<ActiveMonitor> {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: `data` is the `Vec<HMONITOR>` pointer passed below.
        let handles = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
        handles.push(monitor);
        BOOL(1)
    }

    let mut handles: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only runs during this call, while `handles` lives.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
        )
    };
    handles
        .into_iter()
        .filter_map(|handle| {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            // SAFETY: `info` is a `MONITORINFOEXW` with `cbSize` set.
            let ok = unsafe {
                GetMonitorInfoW(handle, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO)
            };
            ok.as_bool().then(|| ActiveMonitor {
                handle,
                gdi_device_name: wide_to_string(&info.szDevice),
                is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            })
        })
        .collect()
}

fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}
