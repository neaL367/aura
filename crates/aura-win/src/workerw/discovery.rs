use std::ptr;

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            EnumChildWindows, EnumWindows, FindWindowExW, FindWindowW, GW_HWNDNEXT, GetClassNameW,
            GetDesktopWindow, GetWindow, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
        },
    },
    core::{BOOL, w},
};

use crate::error::PlatformError;

pub(super) struct ScanResult {
    pub target: Option<HWND>,
}

pub(super) struct ScanContext {
    pub target: Option<HWND>,
}

pub(super) fn find_workerw_pass() -> ScanResult {
    let mut ctx = ScanContext { target: None };

    // SAFETY: EnumWindows passes a valid raw pointer to local stack variable `ctx` via LPARAM.
    unsafe {
        let _ = EnumWindows(Some(find_workerw_callback), LPARAM(&raw mut ctx as isize));
    }

    ScanResult { target: ctx.target }
}

/// Single pass: scan EnumWindows for empty WorkerW or SHELLDLL_DefView host.
pub(super) fn find_workerw_once() -> std::result::Result<HWND, PlatformError> {
    let scan = find_workerw_pass();
    if let Some(target) = scan.target {
        return Ok(target);
    }

    // Direct resolution: Find SHELLDLL_DefView and obtain its host parent window directly.
    // Validate the parent's class and dimensions to avoid attaching to unexpected shell windows.
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetParent;
        let def_hwnd = FindWindowExW(None, None, w!("SHELLDLL_DefView"), None).unwrap_or_default();
        if !def_hwnd.0.is_null() {
            let parent_hwnd = GetParent(def_hwnd).unwrap_or_default();
            let desktop = GetDesktopWindow();
            if parent_hwnd.0.is_null() || parent_hwnd.0 == desktop.0 {
                if parent_hwnd.0 == desktop.0 {
                    tracing::warn!(
                        "SHELLDLL_DefView GetParent() resolved to the raw Desktop Window (HWND {:?}) — rejecting as an invalid attach target",
                        desktop.0
                    );
                }
            } else {
                let mut class_buf = [0u16; 256];
                let len = GetClassNameW(parent_hwnd, &mut class_buf);
                let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
                if class_name == "WorkerW" && is_valid_workerw_candidate(parent_hwnd) {
                    tracing::info!(
                        "SHELLDLL_DefView host window resolved directly (WorkerW): HWND({:?})",
                        parent_hwnd.0
                    );
                    return Ok(parent_hwnd);
                }
                if class_name == "Progman" && is_valid_workerw_candidate(parent_hwnd) {
                    tracing::info!(
                        "SHELLDLL_DefView host window resolved directly (Progman): HWND({:?})",
                        parent_hwnd.0
                    );
                    return Ok(parent_hwnd);
                }
                tracing::warn!(
                    "SHELLDLL_DefView parent HWND({:?}) class '{}' is not a recognized attach target",
                    parent_hwnd.0,
                    class_name
                );
            }
        }
    }

    Err(PlatformError::WorkerWNotFound)
}

/// Send 0x052C to Progman/Desktop, locate the target WorkerW with retry.
pub fn find_and_prepare_workerw() -> std::result::Result<HWND, PlatformError> {
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

    // Step 2: Send 0x052C (idempotent double-dispatch with SMTO_ABORTIFHUNG).
    let mut _result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            target_msg_hwnd,
            0x052C,
            WPARAM(0x0D),
            LPARAM(1),
            SMTO_ABORTIFHUNG,
            500,
            Some(&raw mut _result),
        );
        SendMessageTimeoutW(
            target_msg_hwnd,
            0x052C,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            500,
            Some(&raw mut _result),
        );
    }

    // Step 3: Poll for WorkerW up to ~1.2s (12 attempts x 100ms) for Explorer to split.
    for i in 0..12 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let scan = find_workerw_pass();
        if let Some(target) = scan.target {
            tracing::info!(
                "WorkerW split discovery succeeded (attempt {}/12): found dedicated WorkerW window HWND({:?})",
                i + 1,
                target.0
            );
            return Ok(target);
        }
    }

    // Step 4: Windows 11 24H2+ may keep WorkerW as a child of Progman rather than
    // as a top-level sibling. Search Progman's children for a WorkerW that does
    // NOT contain SHELLDLL_DefView; that is the empty background layer we can host.
    if !progman.0.is_null()
        && let Some(workerw) = find_workerw_child_of_progman(progman)
    {
        tracing::info!(
            "WorkerW child discovery succeeded: found child WorkerW HWND({:?}) under Progman",
            workerw.0
        );
        return Ok(workerw);
    }

    // Step 5: Fallback to Progman for Windows 11 24H2+ composition engine.
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

/// Returns `true` if `hwnd` has client dimensions of at least 300×300.
///
/// Used to filter out tiny internal WorkerW windows that Explorer creates
/// as implementation details and that are unsuitable as wallpaper hosts.
/// Search the children of `progman` for a WorkerW window that does not contain
/// SHELLDLL_DefView. This is the empty background layer used on Windows 11
/// 24H2+ where the WorkerW split is kept as a child of Progman.
fn find_workerw_child_of_progman(progman: HWND) -> Option<HWND> {
    unsafe {
        let mut target: Option<HWND> = None;
        let _ = EnumChildWindows(
            Some(progman),
            Some(find_workerw_child_callback),
            LPARAM(&raw mut target as isize),
        );
        target
    }
}

unsafe extern "system" fn find_workerw_child_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class_buf);
        let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
        if class_name == "WorkerW" {
            let def_view =
                FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None).unwrap_or_default();
            if def_view.0.is_null() {
                let slot = &mut *(lparam.0 as *mut Option<HWND>);
                *slot = Some(hwnd);
                return BOOL::from(false);
            }
        }
    }
    BOOL::from(true)
}

fn is_valid_workerw_candidate(hwnd: HWND) -> bool {
    let mut client_rect = windows::Win32::Foundation::RECT::default();
    let ok = unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client_rect).is_ok()
    };
    if !ok {
        return false;
    }
    let cw = client_rect.right - client_rect.left;
    let ch = client_rect.bottom - client_rect.top;
    cw >= 300 && ch >= 300
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
                // Apply the same size validation as Check 1 to avoid
                // selecting a tiny internal WorkerW as the attach target.
                if !is_valid_workerw_candidate(next_hwnd) {
                    tracing::debug!(
                        "Skipping small sibling WorkerW HWND({:?}) — below 300×300 threshold",
                        next_hwnd.0
                    );
                    next = unsafe { GetWindow(next_hwnd, GW_HWNDNEXT) };
                    continue;
                }
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
