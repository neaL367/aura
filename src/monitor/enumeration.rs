use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW,
    HDC, HMONITOR, MONITORINFOEXW,
};
use crate::utils::error::{AppError, Result};

const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 0x00000001;
const MONITORINFOF_PRIMARY: u32 = 0x00000001;

/// One physically attached display, as seen at enumeration time.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Volatile — invalidated on the next `WM_DISPLAYCHANGE`. Use only as a
    /// same-tick runtime lookup key, never persist it.
    pub hmonitor: HMONITOR,
    /// GDI adapter device name, e.g. `\\.\DISPLAY1`. Also volatile — Windows
    /// can renumber these across reboots or driver changes.
    pub gdi_device_name: String,
    /// Stable hardware identity derived from the monitor's device interface
    /// path (EDID-backed). Safe to persist in config.json as the durable
    /// key for "which physical monitor gets which wallpaper".
    pub device_id: String,
    pub rect: RECT,
    pub work_rect: RECT,
    pub dpi: u32,
    pub is_primary: bool,
}

/// Enumerates all active monitors. Never panics: any single monitor that
/// fails a sub-query (DPI, device id) is logged and skipped rather than
/// aborting the whole enumeration.
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    let mut handles: Vec<HMONITOR> = Vec::new();

    unsafe {
        EnumDisplayMonitors(
            Some(HDC::default()),
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut handles as *mut _ as isize),
        )
        .ok()
        .map_err(|e| AppError::Platform(format!("EnumDisplayMonitors failed: {e}")))?;
    }

    let mut monitors = Vec::with_capacity(handles.len());
    for hmonitor in handles {
        match build_monitor_info(hmonitor) {
            Ok(info) => monitors.push(info),
            Err(e) => {
                tracing::warn!("Skipping monitor {hmonitor:?}: {e}");
            }
        }
    }
    Ok(monitors)
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let handles = &mut *(lparam.0 as *mut Vec<HMONITOR>);
    handles.push(hmonitor);
    BOOL(1)
}

fn build_monitor_info(hmonitor: HMONITOR) -> Result<MonitorInfo> {
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    unsafe {
        GetMonitorInfoW(hmonitor, &mut mi.monitorInfo as *mut _ as *mut _)
            .ok()
            .map_err(|e| AppError::Platform(format!("GetMonitorInfoW failed: {e}")))?;
    }

    let gdi_device_name = wide_to_string(&mi.szDevice);
    let is_primary = (mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;

    let device_id = resolve_stable_device_id(&gdi_device_name)
        // Falling back to the GDI name is better than failing the whole
        // monitor if EDID resolution isn't available (e.g. some virtual
        // displays/RDP sessions don't expose one) — just log it clearly
        // so config persistence knows this id may not survive a reboot.
        .unwrap_or_else(|e| {
            tracing::warn!(
                "No stable device id for {gdi_device_name}, falling back to GDI name: {e}"
            );
            gdi_device_name.clone()
        });

    let (dpi_x, _dpi_y) = get_monitor_dpi(hmonitor)?;

    Ok(MonitorInfo {
        hmonitor,
        gdi_device_name,
        device_id,
        rect: mi.monitorInfo.rcMonitor,
        work_rect: mi.monitorInfo.rcWork,
        dpi: dpi_x,
        is_primary,
    })
}

/// Walks `EnumDisplayDevicesW` twice: once against the adapter (GDI device
/// name) to enumerate its attached monitor, then again with
/// `EDD_GET_DEVICE_INTERFACE_NAME` to get the monitor's own device
/// interface path — an EDID-backed string that stays stable across
/// reboots/driver reloads, unlike `\\.\DISPLAYn` numbering.
fn resolve_stable_device_id(gdi_device_name: &str) -> Result<String> {
    let adapter_name_wide = to_wide(gdi_device_name);

    let mut monitor_device = DISPLAY_DEVICEW::default();
    monitor_device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

    let found = unsafe {
        EnumDisplayDevicesW(
            windows::core::PCWSTR(adapter_name_wide.as_ptr()),
            0, // first monitor on this adapter
            &mut monitor_device,
            EDD_GET_DEVICE_INTERFACE_NAME,
        )
    };

    if !found.as_bool() {
        return Err(AppError::Platform(format!(
            "EnumDisplayDevicesW found no monitor for adapter {gdi_device_name}"
        )));
    }

    let device_id = wide_to_string(&monitor_device.DeviceID);
    if device_id.is_empty() {
        return Err(AppError::Platform(
            "EnumDisplayDevicesW returned an empty DeviceID".into(),
        ));
    }
    Ok(device_id)
}

fn get_monitor_dpi(hmonitor: HMONITOR) -> Result<(u32, u32)> {
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    unsafe {
        GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
            .map_err(|e| AppError::Platform(format!("GetDpiForMonitor failed: {e}")))?;
    }
    Ok((dpi_x, dpi_y))
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_to_string(wide: &[u16]) -> String {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}
