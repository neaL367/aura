use std::ptr;

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, FindWindowExW, FindWindowW, GW_HWNDNEXT, GWL_STYLE, GetClassNameW,
            GetDesktopWindow, GetWindow, GetWindowLongPtrW, SEND_MESSAGE_TIMEOUT_FLAGS, SW_SHOW,
            SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SendMessageTimeoutW, SetParent,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_CHILD, WS_POPUP, WS_VISIBLE,
        },
    },
    core::{BOOL, w},
};

use crate::error::PlatformError;

// ---------------------------------------------------------------------------
// WorkerWManager
// ---------------------------------------------------------------------------

/// Manages the WorkerW attachment lifecycle for a set of host windows.
pub struct WorkerWManager {
    /// Currently known WorkerW HWND. May be null if not yet attached.
    current_workerw: HWND,
}

impl WorkerWManager {
    pub fn new() -> Self {
        Self {
            current_workerw: HWND(ptr::null_mut()),
        }
    }

    /// Find and prepare the WorkerW window handle.
    pub fn find_workerw(&mut self) -> std::result::Result<HWND, PlatformError> {
        let workerw = find_and_prepare_workerw()?;
        self.current_workerw = workerw;
        Ok(workerw)
    }

    /// Find the WorkerW and attach `host_hwnd` to it.
    ///
    /// Idempotent — safe to call repeatedly.
    pub fn ensure_attached(&mut self, host_hwnd: HWND) -> std::result::Result<(), PlatformError> {
        let workerw = self.find_workerw()?;
        attach_to_workerw(host_hwnd, workerw)?;
        Ok(())
    }

    /// Try a single WorkerW discovery pass (no retry). Returns true if attached.
    pub fn try_find_workerw(&mut self) -> bool {
        match find_workerw_once() {
            Ok(workerw) => {
                self.current_workerw = workerw;
                true
            }
            Err(_) => false,
        }
    }

    /// Current WorkerW HWND (null if `ensure_attached` was never called or failed).
    pub fn workerw(&self) -> HWND {
        self.current_workerw
    }
}

impl Default for WorkerWManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Core functions (also used by host_window.rs)
// ---------------------------------------------------------------------------

/// Single pass: scan EnumWindows for empty WorkerW or SHELLDLL_DefView host.
fn find_workerw_once() -> std::result::Result<HWND, PlatformError> {
    let mut found = HWND(ptr::null_mut());
    // SAFETY: EnumWindows passes a valid raw pointer to local stack variable `found` via LPARAM.
    unsafe {
        let _ = EnumWindows(Some(find_workerw_callback), LPARAM(&raw mut found as isize));
    }
    if !found.0.is_null() {
        return Ok(found);
    }

    // Direct resolution: Find SHELLDLL_DefView and obtain its host parent window directly.
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetParent;
        let def_hwnd = FindWindowExW(None, None, w!("SHELLDLL_DefView"), None).unwrap_or_default();
        if !def_hwnd.0.is_null() {
            let parent_hwnd = GetParent(def_hwnd).unwrap_or_default();
            if !parent_hwnd.0.is_null() {
                tracing::info!(
                    "SHELLDLL_DefView host window resolved directly: HWND({:?})",
                    parent_hwnd.0
                );
                return Ok(parent_hwnd);
            }
        }
    }

    Err(PlatformError::WorkerWNotFound)
}

/// Send 0x052C to Progman/Desktop, locate the target WorkerW with retry.
///
/// Polls for the WorkerW up to ~2 seconds (8 × 250ms).
pub(crate) fn find_and_prepare_workerw() -> std::result::Result<HWND, PlatformError> {
    // Step 1: Find Progman or Desktop.
    let mut progman = unsafe { FindWindowExW(None, None, w!("Progman"), None) }.unwrap_or_default();
    if progman.0.is_null() {
        progman = unsafe { FindWindowW(w!("Progman"), None) }.unwrap_or_default();
    }
    if progman.0.is_null() {
        unsafe {
            let _ = EnumWindows(
                Some(find_progman_callback),
                LPARAM(&raw mut progman as isize),
            );
        }
    }

    let target_msg_hwnd = if !progman.0.is_null() {
        progman
    } else {
        unsafe { GetDesktopWindow() }
    };
    tracing::info!(
        "WorkerW discovery starting: Progman HWND({:?}), Target message HWND({:?})",
        progman.0,
        target_msg_hwnd.0
    );

    // Step 2: Send 0x052C (idempotent double-dispatch).
    // Note: WPARAM(0x0D), LPARAM(1) forces Windows 11 desktop composition to spawn/split
    // the secondary WorkerW layer behind icons on newer Windows 11 builds (e.g., 24H2/25H2),
    // followed by standard WPARAM(0), LPARAM(0) for classic Progman composition triggers.
    let mut _result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            target_msg_hwnd,
            0x052C,
            WPARAM(0x0D),
            LPARAM(1),
            SEND_MESSAGE_TIMEOUT_FLAGS(0),
            1000,
            Some(&raw mut _result),
        );
        SendMessageTimeoutW(
            target_msg_hwnd,
            0x052C,
            WPARAM(0),
            LPARAM(0),
            SEND_MESSAGE_TIMEOUT_FLAGS(0),
            1000,
            Some(&raw mut _result),
        );
    }

    // Step 3: Poll for WorkerW up to ~2s.
    for i in 0..8 {
        if let Ok(workerw) = find_workerw_once() {
            tracing::info!(
                "WorkerW split discovery succeeded: found dedicated WorkerW window HWND({:?})",
                workerw.0
            );
            return Ok(workerw);
        }
        if i < 7 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }

    // Step 4: Fallback to Progman / Desktop for Windows 11 24H2+ composition engine.
    let target = if !progman.0.is_null() {
        progman
    } else {
        unsafe { GetDesktopWindow() }
    };
    tracing::warn!(
        "WorkerW split discovery timed out; FALLING BACK to desktop layer HWND({:?}) for Windows 11 24H2/25H2 composition",
        target.0
    );
    Ok(target)
}

unsafe extern "system" fn find_progman_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut class_buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
    if class_name == "Progman" {
        let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
        *slot = hwnd;
        return BOOL::from(false);
    }
    BOOL::from(true)
}

unsafe extern "system" fn check_defview_child_callback(child: HWND, lparam: LPARAM) -> BOOL {
    let mut class_buf = [0u16; 256];
    let len = unsafe { GetClassNameW(child, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
    if class_name == "SHELLDLL_DefView" {
        let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
        *slot = child;
        return BOOL::from(false);
    }
    BOOL::from(true)
}

fn find_defview_child(parent: HWND) -> Option<HWND> {
    let mut found = HWND(ptr::null_mut());
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::EnumChildWindows;
        let _ = EnumChildWindows(
            Some(parent),
            Some(check_defview_child_callback),
            LPARAM(&raw mut found as isize),
        );
    }
    if found.0.is_null() { None } else { Some(found) }
}

/// EnumWindows callback: locates the empty WorkerW below the icon layer.
///
/// # Safety
/// `lparam` must be a valid `*mut HWND` for the duration of `EnumWindows`.
unsafe extern "system" fn find_workerw_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut class_buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);

    let def_view = find_defview_child(hwnd);
    if def_view.is_some() {
        tracing::info!(
            "EnumWindows diagnostic: HWND({:?}) Class='{}' contains SHELLDLL_DefView",
            hwnd.0,
            class_name,
        );

        let mut next = unsafe { GetWindow(hwnd, GW_HWNDNEXT) };
        while let Ok(next_hwnd) = next {
            if next_hwnd.0.is_null() {
                break;
            }
            let mut c_buf = [0u16; 256];
            let c_len = unsafe { GetClassNameW(next_hwnd, &mut c_buf) };
            let c_name = String::from_utf16_lossy(&c_buf[..c_len as usize]);
            if c_name == "WorkerW" {
                let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
                *slot = next_hwnd;
                tracing::info!(
                    "Found target WorkerW sibling directly behind SHELLDLL_DefView parent: HWND({:?})",
                    next_hwnd.0
                );
                return BOOL::from(false);
            }
            next = unsafe { GetWindow(next_hwnd, GW_HWNDNEXT) };
        }

        // If no secondary WorkerW sibling exists (e.g. Windows 11 24H2/25H2 desktop composition engine),
        // target the SHELLDLL_DefView host window itself. Attaching host_hwnd to this parent and placing it at
        // HWND_BOTTOM positions host_hwnd directly behind SHELLDLL_DefView in child Z-order.
        let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
        *slot = hwnd;
        tracing::info!(
            "Windows 11 24H2 composition mode: target host window HWND({:?}) containing SHELLDLL_DefView",
            hwnd.0
        );
        return BOOL::from(false);
    }

    // Fallback Check: Top-level WorkerW window without SHELLDLL_DefView
    if class_name == "WorkerW" {
        let def_view = unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None) };
        let has_def_view = match def_view {
            Ok(h) => !h.0.is_null(),
            Err(_) => false,
        };

        if !has_def_view {
            let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
            *slot = hwnd;
            tracing::info!(
                "Found top-level empty WorkerW (no SHELLDLL_DefView): HWND({:?})",
                hwnd.0
            );
            return BOOL::from(false);
        }
    }

    BOOL::from(true)
}

/// Reparent `host_hwnd` into `workerw` and apply the correct window style.
pub fn attach_to_workerw(host_hwnd: HWND, workerw: HWND) -> std::result::Result<(), PlatformError> {
    unsafe {
        use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
        use windows::Win32::UI::WindowsAndMessaging::{HWND_BOTTOM, SWP_SHOWWINDOW};

        let _ = ShowWindow(workerw, SW_SHOW);

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
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );

        let _ = ShowWindow(host_hwnd, SW_SHOW);
        let _ = UpdateWindow(host_hwnd);
        let _ = InvalidateRect(Some(host_hwnd), None, true);
        let _ = InvalidateRect(Some(workerw), None, true);
    }

    Ok(())
}
