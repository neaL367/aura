/// Win32 full-screen foreground window detection.
#[cfg(target_os = "windows")]
pub fn is_fullscreen_app_active() -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetSystemMetrics, GetWindowRect, SM_CXSCREEN,
        SM_CYSCREEN,
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
                || class_name == "Shell_TrayWnd"
                || class_name == "Windows.UI.Core.CoreWindow"
            {
                return false;
            }
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = (rect.right - rect.left).abs();
            let height = (rect.bottom - rect.top).abs();
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);

            width >= screen_w && height >= screen_h
        } else {
            false
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_fullscreen_app_active() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_fullscreen_app_active_does_not_panic() {
        let _active = is_fullscreen_app_active();
    }
}
