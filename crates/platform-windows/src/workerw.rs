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

struct ScanResult {
    target: Option<HWND>,
    candidates: Vec<isize>,
}

struct ScanContext {
    target: Option<HWND>,
    candidates: Vec<isize>,
}

fn find_workerw_pass() -> ScanResult {
    let mut ctx = ScanContext {
        target: None,
        candidates: Vec::new(),
    };

    // SAFETY: EnumWindows passes a valid raw pointer to local stack variable `ctx` via LPARAM.
    unsafe {
        let _ = EnumWindows(Some(find_workerw_callback), LPARAM(&raw mut ctx as isize));
    }

    ScanResult {
        target: ctx.target,
        candidates: ctx.candidates,
    }
}

/// Single pass: scan EnumWindows for empty WorkerW or SHELLDLL_DefView host.
fn find_workerw_once() -> std::result::Result<HWND, PlatformError> {
    let scan = find_workerw_pass();
    if let Some(target) = scan.target {
        return Ok(target);
    }

    // Direct resolution: Find SHELLDLL_DefView and obtain its host parent window directly.
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetParent;
        let def_hwnd = FindWindowExW(None, None, w!("SHELLDLL_DefView"), None).unwrap_or_default();
        if !def_hwnd.0.is_null() {
            let parent_hwnd = GetParent(def_hwnd).unwrap_or_default();
            let desktop = GetDesktopWindow();
            if !parent_hwnd.0.is_null() && parent_hwnd.0 != desktop.0 {
                tracing::info!(
                    "SHELLDLL_DefView host window resolved directly: HWND({:?})",
                    parent_hwnd.0
                );
                return Ok(parent_hwnd);
            }
            if parent_hwnd.0 == desktop.0 {
                // GetParent() falls back to the desktop window when the queried
                // window has no real parent/owner. A WS_CHILD reparented into the
                // literal desktop window is never composited by DWM — treat this
                // as "not found" rather than accepting an attach target that will
                // silently never render.
                tracing::warn!(
                    "SHELLDLL_DefView GetParent() resolved to the raw Desktop Window (HWND {:?}) — rejecting as an invalid attach target",
                    desktop.0
                );
            }
        }
    }

    Err(PlatformError::WorkerWNotFound)
}

/// Send 0x052C to Progman/Desktop, locate the target WorkerW with retry.
///
/// Polls for the WorkerW up to ~2 seconds (8 × 250ms), automatically
/// short-circuiting if the candidate set remains stable across attempts.
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

    // Step 3: Poll for WorkerW up to ~2s, with candidate set stability detection.
    let mut prev_candidates: Option<Vec<isize>> = None;

    for i in 0..8 {
        let scan = find_workerw_pass();
        if let Some(target) = scan.target {
            tracing::info!(
                "WorkerW split discovery succeeded (attempt {}/8): found dedicated WorkerW window HWND({:?})",
                i + 1,
                target.0
            );
            return Ok(target);
        }

        if prev_candidates.as_ref() == Some(&scan.candidates) {
            tracing::info!(
                "WorkerW candidate set unchanged between attempts {} and {} ({} candidates inspected); short-circuiting to Progman fallback",
                i,
                i + 1,
                scan.candidates.len()
            );
            break;
        }
        prev_candidates = Some(scan.candidates);

        if i < 7 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }

    // Step 4: Fallback to Progman for Windows 11 24H2+ composition engine.
    if !progman.0.is_null() {
        tracing::info!(
            "Falling back to Progman HWND({:?}) for desktop composition",
            progman.0
        );
        return Ok(progman);
    }

    tracing::error!(
        "WorkerW discovery failed completely: no dedicated WorkerW, no SHELLDLL_DefView \
         parent, and no Progman window found. Refusing to fall back to the raw desktop \
         window, since it is never composited by DWM."
    );

    Err(PlatformError::WorkerWNotFound)
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
/// `lparam` must be a valid `*mut ScanContext` for the duration of `EnumWindows`.
unsafe extern "system" fn find_workerw_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam.0 as *mut ScanContext) };

    let mut class_buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);

    if class_name == "WorkerW" || class_name == "Progman" {
        ctx.candidates.push(hwnd.0 as isize);
    }

    // Check 1: Top-level WorkerW window without SHELLDLL_DefView
    if class_name == "WorkerW" {
        let def_view = unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None) };
        let has_def_view = match def_view {
            Ok(h) => !h.0.is_null(),
            Err(_) => false,
        };

        if !has_def_view {
            let mut client_rect = windows::Win32::Foundation::RECT::default();
            let _ = unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect)
            };
            let cw = client_rect.right - client_rect.left;
            let ch = client_rect.bottom - client_rect.top;
            if cw < 300 || ch < 300 {
                tracing::debug!(
                    "Skipping small internal WorkerW candidate HWND({:?}) with rect {}x{}",
                    hwnd.0,
                    cw,
                    ch
                );
            } else {
                ctx.target = Some(hwnd);
                tracing::info!(
                    "Found top-level empty WorkerW (no SHELLDLL_DefView): HWND({:?}) rect {}x{}",
                    hwnd.0,
                    cw,
                    ch
                );
                return BOOL::from(false);
            }
        }
    }

    // Check 2: Window containing SHELLDLL_DefView -> check its Z-order sibling below it
    let def_view = find_defview_child(hwnd);
    if def_view.is_some() {
        tracing::debug!(
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
                ctx.target = Some(next_hwnd);
                tracing::info!(
                    "Found target WorkerW sibling directly behind SHELLDLL_DefView parent: HWND({:?})",
                    next_hwnd.0
                );
                return BOOL::from(false);
            }
            next = unsafe { GetWindow(next_hwnd, GW_HWNDNEXT) };
        }
    }

    BOOL::from(true)
}

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
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );

        let _ = ShowWindow(host_hwnd, SW_SHOW);
        let _ = UpdateWindow(host_hwnd);
        let _ = InvalidateRect(Some(host_hwnd), None, true);
        let _ = InvalidateRect(Some(workerw), None, true);
    }

    Ok(())
}

/// Fallback used when WorkerW/Progman/SHELLDLL_DefView discovery fails entirely.
///
/// Does NOT reparent `host_hwnd` anywhere — keeps it as an ordinary top-level
/// window, positions it at the monitor's real screen coordinates (no
/// `ScreenToClient` needed, since it isn't a child of anything), and pushes it
/// to the bottom of the *top-level* z-order so it sits behind `Progman` and
/// the desktop icons without depending on any particular shell parenting
/// structure.
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
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
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
///
/// Hides split `WorkerW` state and sends `SPI_SETDESKWALLPAPER` refresh so Windows
/// Explorer restores native GDI desktop wallpaper rendering.
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