use std::ptr;

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        Graphics::Gdi::InvalidateRect,
        UI::WindowsAndMessaging::{
            EnumWindows, FindWindowExW, FindWindowW, GWL_STYLE, GetDesktopWindow, GetSystemMetrics,
            GetWindowLongPtrW, MoveWindow, SEND_MESSAGE_TIMEOUT_FLAGS, SM_CXSCREEN, SM_CYSCREEN,
            SW_SHOW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SendMessageTimeoutW, SetParent,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_CHILD, WS_POPUP, WS_VISIBLE,
        },
    },
    core::{BOOL, Result, w},
};

pub fn ensure_attached(render_hwnd: HWND) -> Result<HWND> {
    let mut progman = unsafe { FindWindowW(w!("Progman"), None) }.unwrap_or_default();
    if progman.0.is_null() {
        println!(" [info] FindWindowW(\"Progman\") returned null; searching via EnumWindows...");
        unsafe {
            let _ = EnumWindows(Some(find_progman_proc), LPARAM(&raw mut progman as isize));
        }
    }

    let target_msg_hwnd = if !progman.0.is_null() {
        progman
    } else {
        unsafe { GetDesktopWindow() }
    };
    println!(" Progman / Shell Window : {:?}", target_msg_hwnd.0);

    println!(
        " --- Dumping Progman (0x{:x}) children ---",
        target_msg_hwnd.0 as usize
    );
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::EnumChildWindows;
        let _ = EnumChildWindows(Some(target_msg_hwnd), Some(dump_child_proc), LPARAM(0));
    }

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

    let workerw = match find_workerw_retry() {
        Some(hwnd) => {
            println!(" [✓] Target WorkerW found: {:?}", hwnd.0);
            hwnd
        }
        None => {
            println!(
                " [!] Target WorkerW not found after 2s timeout; falling back to Progman {:?}",
                target_msg_hwnd.0
            );
            target_msg_hwnd
        }
    };

    unsafe {
        use windows::Win32::UI::HiDpi::{DPI_HOSTING_BEHAVIOR_MIXED, SetThreadDpiHostingBehavior};
        let prev = SetThreadDpiHostingBehavior(DPI_HOSTING_BEHAVIOR_MIXED);
        let res = SetParent(render_hwnd, Some(workerw));
        SetThreadDpiHostingBehavior(prev);
        res?;
    }

    unsafe {
        let style = GetWindowLongPtrW(render_hwnd, GWL_STYLE);
        let new_style =
            (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize | WS_VISIBLE.0 as isize;
        SetWindowLongPtrW(render_hwnd, GWL_STYLE, new_style);

        let _ = SetWindowPos(
            render_hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        );

        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        let _ = MoveWindow(render_hwnd, 0, 0, w, h, true);
        let _ = ShowWindow(render_hwnd, SW_SHOW);
        let _ = InvalidateRect(Some(render_hwnd), None, true);
    }

    Ok(workerw)
}

fn find_workerw_retry() -> Option<HWND> {
    for i in 0..8 {
        let hwnd = find_workerw_once();
        if !hwnd.0.is_null() {
            return Some(hwnd);
        }
        if i < 7 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }
    None
}

fn find_workerw_once() -> HWND {
    let mut found = HWND(ptr::null_mut());
    unsafe {
        let _ = EnumWindows(Some(enum_windows_proc), LPARAM(&raw mut found as isize));
    }
    found
}

unsafe extern "system" fn find_progman_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut class_buf = [0u16; 256];
    let len =
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
    if class_name == "Progman" {
        let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
        *slot = hwnd;
        return BOOL::from(false);
    }
    BOOL::from(true)
}

unsafe extern "system" fn dump_child_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let mut class_buf = [0u16; 256];
    let len =
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
    println!(
        " -> Progman Child HWND(0x{:x}) Class='{}'",
        hwnd.0 as usize, class_name
    );
    BOOL::from(true)
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{GW_HWNDNEXT, GetClassNameW, GetWindow};

    let mut class_buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);

    let def_view = unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None) };
    let has_def_view = match def_view {
        Ok(h) => !h.0.is_null(),
        Err(_) => false,
    };

    if class_name == "WorkerW" && !has_def_view {
        let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
        *slot = hwnd;
        return BOOL::from(false);
    }

    if has_def_view {
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
                return BOOL::from(false);
            }
            next = unsafe { GetWindow(next_hwnd, GW_HWNDNEXT) };
        }
    }

    BOOL::from(true)
}
