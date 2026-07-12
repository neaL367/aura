use windows::core::{w, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, FindWindowExW, SendMessageTimeoutW, EnumWindows,
    GetClassNameW, SMTO_NORMAL,
};
use tracing::{info, warn, debug};
use crate::utils::error::{AppError, Result};

struct EnumState {
    shelldll_defview: HWND,
    workerw: HWND,
    total_windows: usize,
}

/// Dispatches the handshake message to Progman to split WorkerW and returns the HWND
/// of the WorkerW window that sits directly behind the desktop icons, as well as the
/// count of top-level windows enumerated (to distinguish headless test sessions).
pub fn get_wallpaper_parent() -> Result<(HWND, usize)> {
    // 1. Run an initial enumeration to check the desktop environment and count windows
    let mut init_state = EnumState {
        shelldll_defview: HWND::default(),
        workerw: HWND::default(),
        total_windows: 0,
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&mut init_state as *mut EnumState as isize),
        );
    }
    let window_count = init_state.total_windows;

    // 2. Find the main Progman window
    let progman = match unsafe { FindWindowW(w!("Progman"), None) } {
        Ok(hwnd) if hwnd != HWND::default() => hwnd,
        _ => {
            return Err(AppError::Platform(format!("Progman window not found (windows={})", window_count)));
        }
    };
    debug!("Found Progman window: {:?}", progman);

    // Look for SHELLDLL_DefView directly under Progman.
    // FindWindowExW(hwndParent, hwndChildAfter, class, title):
    //   hwndParent = `progman` — always a real handle, never null.
    //     (null hwndParent would mean "search all top-level windows" — a different semantic
    //     that we never want here; we are always scoping to a specific window's children.)
    //   hwndChildAfter = Some(HWND::default()) — null sentinel meaning "start from the first child".
    //     In windows 0.62.2 this parameter changed from bare HWND to Option<HWND>;
    //     Some(HWND(0)) is FFI-identical to passing NULL, per MSDN.
    let shelldll = unsafe { FindWindowExW(Some(progman), Some(HWND::default()), w!("SHELLDLL_DefView"), None).unwrap_or_default() };
    if shelldll != HWND::default() {
        debug!("Found SHELLDLL_DefView child under Progman: {:?}", shelldll);
    }

    // 3. Send the message 0x052C to Progman. This is a special undocumented message
    // that tells Windows Explorer to split the desktop background into a separate WorkerW window.
    let mut result: usize = 0;
    let ok = unsafe {
        SendMessageTimeoutW(
            progman,
            0x052C,
            WPARAM(0),
            LPARAM(0),
            SMTO_NORMAL,
            1000,
            Some(&mut result),
        )
    };

    if ok.0 == 0 {
        warn!("SendMessageTimeoutW to Progman returned 0 (failed or timed out)");
    }

    // 4. Enumerate top-level windows again to locate the WorkerW window that does NOT contain
    // the SHELLDLL_DefView. That WorkerW is our target background.
    let mut after_state = EnumState {
        shelldll_defview: if shelldll != HWND::default() { shelldll } else { init_state.shelldll_defview },
        workerw: HWND::default(),
        total_windows: 0,
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&mut after_state as *mut EnumState as isize),
        );
    }

    if after_state.workerw == HWND::default() {
        return Err(AppError::Platform(format!(
            "WorkerW window for wallpaper parent could not be resolved (windows={})",
            window_count
        )));
    }

    info!(
        "Resolved desktop hierarchy: SHELLDLL_DefView = {:?}, WorkerW Parent = {:?}",
        after_state.shelldll_defview, after_state.workerw
    );
    Ok((after_state.workerw, window_count))
}

extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = unsafe { &mut *(lparam.0 as *mut EnumState) };
    state.total_windows += 1;

    // Get the window class name
    let mut class_name = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
    if len > 0 {
        let name_str = String::from_utf16_lossy(&class_name[..len as usize]);
        if name_str == "WorkerW" {
            // hwndParent = `hwnd` (the WorkerW being iterated) — always a real handle, never null.
            // hwndChildAfter = Some(HWND::default()) — null sentinel, "start from first child".
            let shell_dll = unsafe { FindWindowExW(Some(hwnd), Some(HWND::default()), w!("SHELLDLL_DefView"), None).unwrap_or_default() };
            if shell_dll != HWND::default() {
                state.shelldll_defview = shell_dll;
            } else {
                // If it has no SHELLDLL_DefView, it's the desktop wallpaper host spawned by 0x052C.
                state.workerw = hwnd;
            }
        }
    }

    BOOL(1) // Keep enumerating
}
