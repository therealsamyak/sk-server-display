//! Windows display helpers: list active displays with their output kind,
//! and find the physical status display.

use std::{collections::HashMap, ptr::without_provenance_mut, rc::Rc};

use gpui::PlatformDisplay;
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
    Foundation::ERROR_SUCCESS,
    Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFOEXW},
    UI::WindowsAndMessaging::MONITORINFOF_PRIMARY,
};

use crate::platform::status_display::{
    DisplayOutputKind, PhysicalDisplayCandidate, rank_physical_display_candidates,
};

struct ActiveDisplayPath {
    gdi_device_name: String,
    friendly_name: String,
    kind: DisplayOutputKind,
}

impl ActiveDisplayPath {
    fn display_description(&self) -> String {
        let name = if self.friendly_name.is_empty() {
            self.gdi_device_name.trim_start_matches(r"\\.\")
        } else {
            &self.friendly_name
        };
        format!("{} \"{name}\"", self.kind.display_kind_label())
    }
}

/// Ranks active Windows paths for displays already enumerated by GPUI.
pub(super) fn find_physical_windows_display(
    displays: &[Rc<dyn PlatformDisplay>],
) -> Option<(&Rc<dyn PlatformDisplay>, String)> {
    let paths = query_active_display_paths()?;
    let monitors: HashMap<_, _> = displays
        .iter()
        .filter_map(|display| {
            let info = display_monitor_info(display.as_ref())?;
            let name = wide_to_string(&info.szDevice);
            let is_primary = info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0;
            Some((name, (display, is_primary)))
        })
        .collect();
    let (candidates, matches): (Vec<_>, Vec<_>) = paths
        .iter()
        .filter_map(|path| {
            let &(display, is_primary) = monitors.get(&path.gdi_device_name)?;
            Some((
                PhysicalDisplayCandidate {
                    kind: path.kind,
                    is_primary,
                },
                (display, path),
            ))
        })
        .unzip();
    let &(display, path) = matches.get(rank_physical_display_candidates(&candidates)?)?;
    Some((display, path.display_description()))
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

#[expect(
    unsafe_code,
    reason = "Win32 display paths identify physical and virtual outputs"
)]
fn query_active_display_paths() -> Option<Vec<ActiveDisplayPath>> {
    // Temporary unsafe for connector types and names. wgpu uses the same Win32 queries.
    // https://github.com/gfx-rs/wgpu/blob/60cef2be828e03ba54e2e1598001b4625c0c2163/wgpu-hal/src/auxil/dxgi/hdr.rs#L296-L351
    let mut path_count = 0u32;
    let mut mode_count = 0u32;
    // SAFETY: both counts are initialized, writable u32s.
    let status = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count.max(1) as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count.max(1) as usize];
    // SAFETY: both initialized buffers cover the mutable element counts.
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
        // SAFETY: the pointer covers the writable target packet and its size-tagged header.
        if unsafe { DisplayConfigGetDeviceInfo((&raw mut target).cast()) } != 0 {
            continue;
        }
        // SAFETY: the pointer covers the writable source packet and its size-tagged header.
        if unsafe { DisplayConfigGetDeviceInfo((&raw mut source).cast()) } != 0 {
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

#[expect(
    unsafe_code,
    reason = "GPUI does not expose the Windows GDI device name"
)]
fn display_monitor_info(display: &dyn PlatformDisplay) -> Option<MONITORINFOEXW> {
    // The pinned GPUI Windows backend stores the HMONITOR value as its display ID.
    let handle = HMONITOR(without_provenance_mut(
        usize::try_from(u64::from(display.id())).ok()?,
    ));
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = u32::try_from(size_of::<MONITORINFOEXW>()).ok()?;
    // Temporary unsafe to match monitors by GDI name. Zed uses the same Win32 query.
    // https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/display.rs#L214-L223
    // SAFETY: GPUI supplied the monitor handle, and cbSize covers the full output.
    let ok = unsafe { GetMonitorInfoW(handle, (&raw mut info).cast()) };
    ok.as_bool().then_some(info)
}

fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}
