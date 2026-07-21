use std::ptr;

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, FindWindowExW, FindWindowW, GWL_STYLE, GetWindowLongPtrW,
            SEND_MESSAGE_TIMEOUT_FLAGS, SW_SHOW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE,
            SendMessageTimeoutW, SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_CHILD,
            WS_POPUP, WS_VISIBLE,
        },
    },
    core::{BOOL, w},
};

use crate::error::PlatformError;

// ---------------------------------------------------------------------------
// WorkerWManager
// ---------------------------------------------------------------------------

/// Manages the WorkerW attachment lifecycle for a set of host windows.
///
/// # Algorithm (same as workerw-proof tool — now production quality)
///
/// 1. `FindWindow("Progman")` → locate Program Manager.
/// 2. `SendMessageTimeout(progman, 0x052C, …)` → trigger WorkerW insertion.
/// 3. `EnumWindows` → find the empty WorkerW (below SHELLDLL_DefView layer).
/// 4. `SetParent(hwnd, workerw)` for each host window.
/// 5. On `TaskbarCreated`: repeat steps 1–4 with fresh host windows.
pub struct WorkerWManager {
    /// Currently known WorkerW HWND.  May be null if not yet attached.
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

/// Single pass: send 0x052C and poll EnumWindows once.
fn find_workerw_once() -> std::result::Result<HWND, PlatformError> {
    let mut found = HWND(ptr::null_mut());
    unsafe {
        let _ = EnumWindows(Some(find_workerw_callback), LPARAM(&raw mut found as isize));
    }
    if found.0.is_null() {
        Err(PlatformError::WorkerWNotFound)
    } else {
        Ok(found)
    }
}

/// Send 0x052C to Progman, locate the target WorkerW with retry.
///
/// Polls for the WorkerW up to ~2 seconds (8 × 250ms).
pub(crate) fn find_and_prepare_workerw() -> std::result::Result<HWND, PlatformError> {
    // Step 1: Find Progman (once — it doesn't vanish during our retry window).
    let progman = unsafe { FindWindowW(w!("Progman"), None) }?;
    if progman.0.is_null() {
        return Err(PlatformError::WorkerWNotFound);
    }

    // Step 2: Send 0x052C (idempotent).
    let mut _result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            progman,
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
            return Ok(workerw);
        }
        if i < 7 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }

    Err(PlatformError::WorkerWNotFound)
}

/// EnumWindows callback: locates the empty WorkerW below the icon layer.
///
/// # Safety
/// `lparam` must be a valid `*mut HWND` for the duration of `EnumWindows`.
unsafe extern "system" fn find_workerw_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let def_view = unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None) };
    if def_view.is_err() {
        return BOOL::from(true);
    }
    let target = unsafe { FindWindowExW(None, Some(hwnd), w!("WorkerW"), None) };
    if let Ok(target_hwnd) = target {
        let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
        *slot = target_hwnd;
    }
    BOOL::from(true)
}

/// Reparent `host_hwnd` into `workerw` and apply the correct window style.
pub fn attach_to_workerw(
    host_hwnd: HWND,
    workerw: HWND,
) -> std::result::Result<(), PlatformError> {
    unsafe {
        SetParent(host_hwnd, Some(workerw))?;

        // Update style: remove WS_POPUP, add WS_CHILD.
        let style = GetWindowLongPtrW(host_hwnd, GWL_STYLE);
        let new_style =
            (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize | WS_VISIBLE.0 as isize;
        SetWindowLongPtrW(host_hwnd, GWL_STYLE, new_style);

        let _ = SetWindowPos(
            host_hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        );
        let _ = ShowWindow(host_hwnd, SW_SHOW);
    }
    Ok(())
}
