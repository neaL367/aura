use std::ptr;

use windows::{
    Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST,
            MONITORINFOEXW, MonitorFromWindow,
        },
        UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    },
    core::BOOL,
};

use aura_core::monitor::{MonitorId, MonitorInfo};

use crate::error::PlatformError;

// ---------------------------------------------------------------------------
// MonitorEnumerator
// ---------------------------------------------------------------------------

/// Enumerates connected monitors and produces stable `MonitorInfo` snapshots.
pub struct MonitorEnumerator;

impl MonitorEnumerator {
    /// Enumerate all currently connected monitors.
    ///
    /// Returns a sorted list (primary monitor first).
    pub fn enumerate() -> Result<Vec<MonitorInfo>, PlatformError> {
        let mut infos: Vec<MonitorInfo> = Vec::new();

        // SAFETY: EnumDisplayMonitors callback receives valid HMONITOR handles.
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(enum_monitor_callback),
                LPARAM(&raw mut infos as isize),
            )
            .ok()
            .map_err(|e| PlatformError::MonitorEnum(e.to_string()))?;
        }

        if infos.is_empty() {
            return Err(PlatformError::NoMonitors);
        }

        // Primary monitor first.
        infos.sort_by_key(|m| if m.is_primary { 0i32 } else { 1i32 });
        Ok(infos)
    }
}

/// EnumDisplayMonitors callback.
///
/// # Safety
/// `lparam` is a valid `*mut Vec<MonitorInfo>` for the duration of enumeration.
unsafe extern "system" fn enum_monitor_callback(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let infos = unsafe { &mut *(lparam.0 as *mut Vec<MonitorInfo>) };

    let mut minfo: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    minfo.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if unsafe { GetMonitorInfoW(hmonitor, &mut minfo.monitorInfo).as_bool() } {
        // Device name: the szDevice field is a wide string (null-terminated).
        let device_name = String::from_utf16_lossy(
            &minfo.szDevice[..minfo.szDevice.iter().position(|&c| c == 0).unwrap_or(32)],
        );

        // Derive stable MonitorId from device name.
        // For a truly unique path, a full SetupAPI traversal is needed; the
        // device name is sufficient for practical uniqueness in most setups.
        let monitor_id = MonitorId::from_device_path(&device_name);

        let rc = minfo.monitorInfo.rcMonitor;
        let width = (rc.right - rc.left) as u32;
        let height = (rc.bottom - rc.top) as u32;
        let is_primary = (minfo.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY

        // Query effective DPI.
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        unsafe {
            let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        }

        infos.push(MonitorInfo {
            id: monitor_id,
            device_name: device_name.clone(),
            device_path: device_name,
            x: rc.left,
            y: rc.top,
            width,
            height,
            dpi: dpi_x,
            is_primary,
        });
    }

    BOOL::from(true) // continue enumeration
}
