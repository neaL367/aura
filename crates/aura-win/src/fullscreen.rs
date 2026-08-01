/// Win32 full-screen foreground window detection.
///
/// Multi-monitor aware: compares the foreground window rect against the
/// monitor the window occupies (`MonitorFromWindow` + `GetMonitorInfoW`)
/// rather than the primary screen metrics. A ~99% coverage tolerance is
/// applied because borderless fullscreen windows can differ by 1-2 px due to
/// DPI scaling, invisible resize borders, or compositor behavior.
#[cfg(target_os = "windows")]
pub fn is_fullscreen_app_active() -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        // Filter out desktop shell windows
        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_buf);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            if class_name == "Progman"
                || class_name == "WorkerW"
                || class_name == "Shell_TrayW"
                || class_name == "Windows.UI.Core.CoreWindow"
            {
                return false;
            }
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        let win_w = (rect.right - rect.left).abs() as i64;
        let win_h = (rect.bottom - rect.top).abs() as i64;

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return false;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let mon_w = (info.rcMonitor.right - info.rcMonitor.left).abs() as i64;
        let mon_h = (info.rcMonitor.bottom - info.rcMonitor.top).abs() as i64;

        // ~99% coverage tolerance (see module docs).
        win_w >= (mon_w * 99) / 100 && win_h >= (mon_h * 99) / 100
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_fullscreen_app_active() -> bool {
    false
}
