//! Windows display helpers: list active displays with their output kind,
//! and find the physical status display.

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
    Foundation::{ERROR_SUCCESS, LPARAM, RECT},
    Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    },
    UI::WindowsAndMessaging::MONITORINFOF_PRIMARY,
};
use windows::core::BOOL;

use crate::platform::status_display::{
    DisplayOutputKind, PhysicalDisplayCandidate, rank_physical_display_candidates,
};

/// One active Windows display: its monitor handle, desktop rectangle, and
/// output kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsDisplay {
    /// `HMONITOR` value. GPUI on Windows uses it as the display ID.
    pub monitor_handle: isize,
    /// GDI device name, such as `\\.\DISPLAY1`.
    pub gdi_device_name: String,
    /// Monitor name from the display configuration, such as `Built-in Display`.
    pub friendly_name: String,
    pub kind: DisplayOutputKind,
    pub is_primary: bool,
    /// Top-left corner in physical desktop pixels.
    pub position: (i32, i32),
    /// Size in physical pixels.
    pub size: (u32, u32),
}

impl WindowsDisplay {
    /// Display label detail, such as `built-in panel "Built-in Display"`.
    pub fn windows_display_description(&self) -> String {
        let name = if self.friendly_name.is_empty() {
            self.gdi_device_name.trim_start_matches(r"\\.\")
        } else {
            &self.friendly_name
        };
        format!("{} \"{name}\"", self.kind.display_kind_label())
    }
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
    rect: RECT,
}

/// Lists the active displays that have both a display path and a monitor.
pub fn list_windows_displays() -> Vec<WindowsDisplay> {
    let Some(paths) = query_active_display_paths() else {
        return Vec::new();
    };
    let monitors = enumerate_active_monitors();
    paths
        .into_iter()
        .filter_map(|path| {
            let monitor = monitors
                .iter()
                .find(|monitor| monitor.gdi_device_name == path.gdi_device_name)?;
            Some(WindowsDisplay {
                monitor_handle: monitor.handle.0 as isize,
                gdi_device_name: path.gdi_device_name,
                friendly_name: path.friendly_name,
                kind: path.kind,
                is_primary: monitor.is_primary,
                position: (monitor.rect.left, monitor.rect.top),
                size: (
                    (monitor.rect.right - monitor.rect.left).max(0) as u32,
                    (monitor.rect.bottom - monitor.rect.top).max(0) as u32,
                ),
            })
        })
        .collect()
}

/// Finds the physical status display: the built-in panel first, then a
/// physical external monitor. Virtual displays are never returned.
pub fn find_physical_windows_display(displays: &[WindowsDisplay]) -> Option<&WindowsDisplay> {
    let candidates: Vec<PhysicalDisplayCandidate> = displays
        .iter()
        .map(|display| PhysicalDisplayCandidate {
            kind: display.kind,
            is_primary: display.is_primary,
        })
        .collect();
    displays.get(rank_physical_display_candidates(&candidates)?)
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
                rect: info.monitorInfo.rcMonitor,
            })
        })
        .collect()
}

fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}
