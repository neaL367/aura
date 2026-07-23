use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SW_SHOW, SetParent, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, WS_CHILD, WS_POPUP, WS_VISIBLE,
    },
};

use crate::error::PlatformError;

struct ScopedDpiHostingBehavior {
    previous: windows::Win32::UI::HiDpi::DPI_HOSTING_BEHAVIOR,
}

impl ScopedDpiHostingBehavior {
    pub fn allow_mixed() -> Self {
        use windows::Win32::UI::HiDpi::{DPI_HOSTING_BEHAVIOR_MIXED, SetThreadDpiHostingBehavior};
        let previous = unsafe { SetThreadDpiHostingBehavior(DPI_HOSTING_BEHAVIOR_MIXED) };
        Self { previous }
    }
}

impl Drop for ScopedDpiHostingBehavior {
    fn drop(&mut self) {
        use windows::Win32::UI::HiDpi::SetThreadDpiHostingBehavior;
        unsafe {
            SetThreadDpiHostingBehavior(self.previous);
        }
    }
}

/// Reparent `host_hwnd` into `workerw` and apply the correct window style.
pub fn attach_to_workerw(host_hwnd: HWND, workerw: HWND) -> std::result::Result<(), PlatformError> {
    unsafe {
        use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
        use windows::Win32::UI::WindowsAndMessaging::{HWND_BOTTOM, SWP_SHOWWINDOW};

        let mut class_buf = [0u16; 256];
        let len = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(workerw, &mut class_buf);
        let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);

        let mut client_rect = windows::Win32::Foundation::RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(workerw, &mut client_rect);
        let client_w = client_rect.right - client_rect.left;
        let client_h = client_rect.bottom - client_rect.top;

        let visible = windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(workerw).as_bool();

        tracing::info!(
            "Attach target class='{}', hwnd={:?}, client_rect={}x{}, visible={}",
            class_name,
            workerw.0,
            client_w,
            client_h,
            visible
        );

        let _ = ShowWindow(workerw, SW_SHOW);

        let _dpi_guard = ScopedDpiHostingBehavior::allow_mixed();

        // Set WorkerW class background brush to BLACK_BRUSH so empty/unpainted
        // WorkerW surfaces erase to black instead of DWM default white if wallpaperd is killed.
        use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject};
        use windows::Win32::UI::WindowsAndMessaging::{GCLP_HBRBACKGROUND, SetClassLongPtrW};
        let black_brush = GetStockObject(BLACK_BRUSH);
        let _ = SetClassLongPtrW(workerw, GCLP_HBRBACKGROUND, black_brush.0 as isize);
        SetParent(host_hwnd, Some(workerw))?;

        let style = GetWindowLongPtrW(host_hwnd, GWL_STYLE);
        let new_style =
            (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize | WS_VISIBLE.0 as isize;
        SetWindowLongPtrW(host_hwnd, GWL_STYLE, new_style);
        use windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE;
        SetWindowLongPtrW(host_hwnd, GWL_EXSTYLE, 0);

        let _ = SetWindowPos(
            host_hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                | windows::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED
                | SWP_SHOWWINDOW,
        );

        let _ = ShowWindow(host_hwnd, SW_SHOW);
        let _ = UpdateWindow(host_hwnd);
        let _ = InvalidateRect(Some(host_hwnd), None, true);
        let _ = InvalidateRect(Some(workerw), None, true);
    }

    Ok(())
}

/// Fallback used when WorkerW/Progman/SHELLDLL_DefView discovery fails entirely.
pub fn attach_topmost_bottom(
    host_hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> std::result::Result<(), PlatformError> {
    use windows::Win32::Graphics::Gdi::InvalidateRect;
    use windows::Win32::UI::WindowsAndMessaging::{HWND_BOTTOM, MoveWindow, SWP_SHOWWINDOW};

    unsafe {
        let _ = MoveWindow(host_hwnd, x, y, width, height, true);
        let _ = SetWindowPos(
            host_hwnd,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                | windows::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED
                | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(host_hwnd, SW_SHOW);
        let _ = InvalidateRect(Some(host_hwnd), None, true);
    }

    tracing::warn!(
        "Using top-level (unparented) fallback placement for HWND({:?}) — WorkerW/Progman discovery did not resolve a valid attach target",
        host_hwnd.0
    );

    Ok(())
}

/// Restore Windows native desktop wallpaper rendering.
pub fn restore_desktop_wallpaper() {
    unsafe {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::Graphics::Gdi::InvalidateRect;
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowExW, FindWindowW, SEND_MESSAGE_TIMEOUT_FLAGS, SPI_SETDESKWALLPAPER,
            SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SendMessageTimeoutW, SystemParametersInfoW,
        };
        use windows::core::w;

        let mut progman = FindWindowExW(None, None, w!("Progman"), None).unwrap_or_default();
        if progman.0.is_null() {
            progman = FindWindowW(w!("Progman"), None).unwrap_or_default();
        }
        if !progman.0.is_null() {
            let mut res = 0usize;
            let _ = SendMessageTimeoutW(
                progman,
                0x052C,
                WPARAM(0),
                LPARAM(0),
                SEND_MESSAGE_TIMEOUT_FLAGS(0),
                1000,
                Some(&raw mut res),
            );
            let _ = InvalidateRect(Some(progman), None, true);
        }

        let _ = SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            None,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
    }
}
