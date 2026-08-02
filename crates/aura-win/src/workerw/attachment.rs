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
        if let Err(e) =
            windows::Win32::UI::WindowsAndMessaging::GetClientRect(workerw, &mut client_rect)
        {
            tracing::warn!("GetClientRect failed for WorkerW {:?}: {}", workerw.0, e);
        }
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

        // Windows 11 24H2+ sometimes exposes the Progman window itself as the
        // desktop composition layer, with no usable WorkerW sibling. Progman
        // cannot be used as a parent for reparenting; instead the host must be
        // a top-level window placed behind the desktop.
        let target_is_progman = class_name == "Progman";
        if target_is_progman {
            return attach_topmost_bottom(host_hwnd, 0, 0, client_w, client_h, Some(workerw));
        }

        let _dpi_guard = ScopedDpiHostingBehavior::allow_mixed();

        // Do NOT call SetClassLongPtrW(workerw, GCLP_HBRBACKGROUND, ...) here.
        // That would mutate the background brush of Explorer's WorkerW window
        // class — a global side effect that persists after Aura exits and
        // affects all WorkerW instances of the same class, not just this one.
        // The Aura host window's background is controlled by the Vulkan
        // renderer clearing to black on each present.
        // SetParent returns the previous parent. A top-level host has no
        // previous parent, so windows-rs can report NULL as an error even
        // though reparenting succeeded. Verify the new parent explicitly.
        // A top-level WS_POPUP window cannot be reparented into a child
        // WorkerW. Switch to WS_CHILD before SetParent so the host becomes a
        // true child window of the target shell layer.
        let style = GetWindowLongPtrW(host_hwnd, GWL_STYLE);
        let child_style =
            (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize | WS_VISIBLE.0 as isize;
        SetWindowLongPtrW(host_hwnd, GWL_STYLE, child_style);
        use windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE;
        SetWindowLongPtrW(host_hwnd, GWL_EXSTYLE, 0);

        let set_parent_error = SetParent(host_hwnd, Some(workerw)).err();
        let actual_parent =
            windows::Win32::UI::WindowsAndMessaging::GetParent(host_hwnd).unwrap_or_default();
        if actual_parent != workerw {
            return Err(PlatformError::Win32(
                set_parent_error.unwrap_or_else(windows::core::Error::from_thread),
            ));
        }
        if let Some(error) = set_parent_error {
            tracing::debug!(
                "SetParent reported previous-parent error after successful reparent: {}",
                error
            );
        }

        use windows::Win32::UI::WindowsAndMessaging::FindWindowExW;
        use windows::core::w;

        let def_view =
            FindWindowExW(Some(workerw), None, w!("SHELLDLL_DefView"), None).unwrap_or_default();
        let insert_after = if !def_view.0.is_null() {
            def_view
        } else {
            HWND_BOTTOM
        };

        if let Err(e) = SetWindowPos(
            host_hwnd,
            Some(insert_after),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                | windows::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED
                | SWP_SHOWWINDOW,
        ) {
            tracing::warn!(
                "SetWindowPos failed for host window {:?}: {}",
                host_hwnd.0,
                e
            );
        }

        let _ = ShowWindow(host_hwnd, SW_SHOW);
        let _ = UpdateWindow(host_hwnd);
        let _ = InvalidateRect(Some(host_hwnd), None, true);
        let _ = InvalidateRect(Some(workerw), None, true);
    }

    Ok(())
}

/// Fallback used when WorkerW/Progman/SHELLDLL_DefView discovery fails entirely,
/// or when the shell target is Progman (Windows 11 24H2+) and cannot be reparented.
///
/// `insert_after` is the shell window to place the host behind. When supplied,
/// the host is placed directly behind that window; otherwise it is placed at
/// the bottom of the top-level Z-order. `SWP_NOACTIVATE` is used so the host
/// does not steal foreground and cover the desktop UI.
pub fn attach_topmost_bottom(
    host_hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    insert_after: Option<HWND>,
) -> std::result::Result<(), PlatformError> {
    use windows::Win32::Graphics::Gdi::InvalidateRect;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, HWND_BOTTOM, MoveWindow, SW_SHOWNA, SWP_NOACTIVATE, SWP_SHOWWINDOW,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    unsafe {
        let style = GetWindowLongPtrW(host_hwnd, GWL_STYLE);
        let new_style =
            (style & !(WS_CHILD.0 as isize)) | WS_POPUP.0 as isize | WS_VISIBLE.0 as isize;
        SetWindowLongPtrW(host_hwnd, GWL_STYLE, new_style);
        let ex_style = WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
        SetWindowLongPtrW(host_hwnd, GWL_EXSTYLE, ex_style);

        if let Err(e) = MoveWindow(host_hwnd, x, y, width, height, true) {
            tracing::warn!(
                "MoveWindow fallback failed for host window {:?}: {}",
                host_hwnd.0,
                e
            );
        }
        let after = insert_after.unwrap_or(HWND_BOTTOM);
        if let Err(e) = SetWindowPos(
            host_hwnd,
            Some(after),
            0,
            0,
            0,
            0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                | windows::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED
                | SWP_SHOWWINDOW
                | SWP_NOACTIVATE,
        ) {
            tracing::warn!(
                "SetWindowPos fallback failed for host window {:?}: {}",
                host_hwnd.0,
                e
            );
        }
        let _ = ShowWindow(host_hwnd, SW_SHOWNA);
        let _ = InvalidateRect(Some(host_hwnd), None, true);
    }

    tracing::info!(
        "Placing host window HWND({:?}) as top-level behind the desktop shell",
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
            FindWindowExW, FindWindowW, SEND_MESSAGE_TIMEOUT_FLAGS, SPI_GETDESKWALLPAPER,
            SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
            SendMessageTimeoutW, SystemParametersInfoW,
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

        // Query current Windows desktop wallpaper path first so we can trigger a refresh
        // without replacing the wallpaper with Windows default or mutating registry.
        let mut path_buf = [0u16; 260];
        if SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            path_buf.len() as u32,
            Some(path_buf.as_mut_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            let len = path_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(path_buf.len());
            if len > 0 {
                let _ = SystemParametersInfoW(
                    SPI_SETDESKWALLPAPER,
                    0,
                    Some(path_buf.as_ptr() as *mut _),
                    SPIF_SENDCHANGE,
                );
            }
        }
    }
}
