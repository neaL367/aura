//! Phase 0 — WorkerW Desktop Integration Proof — Windows implementation
//!
//! **Purpose**: Verify the WorkerW attachment mechanism used by `wallpaperd`
//! before introducing Vulkan complexity. This is a throwaway validation tool.

use std::cell::Cell;
use std::mem;
use std::ptr;

use windows::{
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateSolidBrush, EndPaint, FillRect, HBRUSH, InvalidateRect, PAINTSTRUCT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows,
            FindWindowExW, FindWindowW, GWL_STYLE, GetClientRect, GetMessageW, GetSystemMetrics,
            GetWindowLongPtrW, IDC_ARROW, LoadCursorW, MSG, MoveWindow, PostQuitMessage,
            RegisterClassExW, RegisterWindowMessageW, SEND_MESSAGE_TIMEOUT_FLAGS, SM_CXSCREEN,
            SM_CYSCREEN, SW_SHOW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SendMessageTimeoutW,
            SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
            WINDOW_EX_STYLE, WM_DESTROY, WM_DISPLAYCHANGE, WM_PAINT, WNDCLASSEXW, WS_CHILD,
            WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP, WS_VISIBLE,
        },
    },
    core::{BOOL, Error, HRESULT, Result, w},
};

// ---------------------------------------------------------------------------
// Window class name literals (compile-time wide strings)
// ---------------------------------------------------------------------------

const CLASS_CONTROL: windows::core::PCWSTR = w!("AuraProof_Control");
const CLASS_RENDER: windows::core::PCWSTR = w!("AuraProof_Render");

// ---------------------------------------------------------------------------
// Thread-local state
// ---------------------------------------------------------------------------

thread_local! {
    /// Message ID for "TaskbarCreated" — registered at startup, stable per session.
    static TASKBAR_MSG_ID: Cell<u32> = const { Cell::new(0) };

    /// Current render window HWND (as isize). Replaced after each Explorer restart.
    static RENDER_HWND_RAW: Cell<isize> = const { Cell::new(0) };

    /// HINSTANCE for this process — needed to recreate windows from WndProc.
    static HINSTANCE_RAW: Cell<isize> = const { Cell::new(0) };

    /// Total successful WorkerW attachments (for proof output).
    static ATTACH_COUNT: Cell<u32> = const { Cell::new(0) };
}

// ---------------------------------------------------------------------------
// Helpers: convert between isize and typed handles
// ---------------------------------------------------------------------------

#[inline]
fn render_hwnd() -> HWND {
    HWND(RENDER_HWND_RAW.with(Cell::get) as *mut _)
}

#[inline]
fn process_hinstance() -> HINSTANCE {
    HINSTANCE(HINSTANCE_RAW.with(Cell::get) as *mut _)
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

pub fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║  Aura — WorkerW Desktop Integration Proof (Phase 0)  ║");
    println!("╚══════════════════════════════════════════════════════╝");
    println!();
    println!("Expected : A solid RED rectangle behind desktop icons.");
    println!("Recovery : Restart Explorer (Task Manager → restart) to");
    println!("           test automatic re-attachment.");
    println!("Exit     : Close this console window.\n");

    // Obtain the process HINSTANCE.
    //
    // SAFETY: GetModuleHandleW(None) always succeeds for the current executable.
    let hmodule = unsafe { GetModuleHandleW(None)? };
    let hinstance = HINSTANCE(hmodule.0);
    HINSTANCE_RAW.with(|c| c.set(hinstance.0 as isize));

    // Register the "TaskbarCreated" shell broadcast message.
    //
    // Explorer sends this to HWND_BROADCAST when it (re)starts.  The returned
    // ID is unique per session and lies in 0xC000–0xFFFF.
    //
    // SAFETY: RegisterWindowMessageW is always safe with a valid PCWSTR.
    let taskbar_msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    if taskbar_msg == 0 {
        return Err(Error::from_thread());
    }
    TASKBAR_MSG_ID.with(|c| c.set(taskbar_msg));
    println!("TaskbarCreated message ID : 0x{:04X}\n", taskbar_msg);

    // Register both window classes.
    register_classes(hinstance)?;

    // Create the top-level control window.
    //
    // This window is never shown.  Because it has no parent (`None` → desktop
    // parent) it receives HWND_BROADCAST messages, including TaskbarCreated.
    //
    // SAFETY: CreateWindowExW — all parameters are valid.
    let _control_hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_CONTROL,
            w!("AuraProof_Control"),
            WS_POPUP | WS_CLIPCHILDREN,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinstance),
            None,
        )?
    };
    println!("Control window : {:?}", _control_hwnd.0);

    // Create the initial render window and attach it to WorkerW.
    let render_hwnd = create_and_attach(hinstance)?;
    RENDER_HWND_RAW.with(|c| c.set(render_hwnd.0 as isize));

    // -----------------------------------------------------------------------
    // Message loop
    // -----------------------------------------------------------------------
    println!("\nMessage loop running…");
    let mut msg = MSG::default();
    loop {
        // SAFETY: GetMessageW fills `msg`; None hwnd = all messages for this thread.
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match r.0 {
            -1 => return Err(Error::from_thread()),
            0 => break, // WM_QUIT
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }

    println!(
        "\nExited. Total successful attachments: {}",
        ATTACH_COUNT.with(Cell::get)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Window class registration
// ---------------------------------------------------------------------------

fn register_classes(hinstance: HINSTANCE) -> Result<()> {
    // SAFETY: LoadCursorW with None loads a system resource; IDC_ARROW always exists.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };

    // Control class — no background brush (never painted).
    let control_wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(control_wnd_proc),
        hInstance: hinstance,
        hCursor: cursor,
        lpszClassName: CLASS_CONTROL,
        ..Default::default()
    };
    // SAFETY: RegisterClassExW with a fully initialised WNDCLASSEXW.
    if unsafe { RegisterClassExW(&control_wc) } == 0 {
        return Err(Error::from_thread());
    }

    // Render class — red background brush proves visibility.
    //
    // COLORREF is BGR: 0x0000_00FF = pure red (R=255, G=0, B=0).
    //
    // SAFETY: CreateSolidBrush always returns a valid HBRUSH or null.
    let brush: HBRUSH = unsafe { CreateSolidBrush(COLORREF(0x0000_00FF)) };
    if brush.is_invalid() {
        return Err(Error::from_thread());
    }

    let render_wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(render_wnd_proc),
        hInstance: hinstance,
        hCursor: cursor,
        hbrBackground: brush,
        lpszClassName: CLASS_RENDER,
        ..Default::default()
    };
    // SAFETY: RegisterClassExW with a fully initialised WNDCLASSEXW.
    if unsafe { RegisterClassExW(&render_wc) } == 0 {
        return Err(Error::from_thread());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// create_and_attach — unit of work repeated on each Explorer restart
// ---------------------------------------------------------------------------

fn create_and_attach(hinstance: HINSTANCE) -> Result<HWND> {
    // SAFETY: CreateWindowExW — all parameters are valid.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_RENDER,
            w!("AuraProof_Render"),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            800,
            600,
            None,
            None,
            Some(hinstance),
            None,
        )?
    };
    println!("[+] Render window created : {:?}", hwnd.0);

    match ensure_attached(hwnd) {
        Ok(workerw) => {
            let n = ATTACH_COUNT.with(|c| {
                let v = c.get() + 1;
                c.set(v);
                v
            });
            println!(
                "[✓] Attached render {:?} → WorkerW {:?}  (attach #{})",
                hwnd.0, workerw.0, n
            );
        }
        Err(e) => {
            eprintln!("[✗] Attachment failed: {}", e);
        }
    }

    Ok(hwnd)
}

// ---------------------------------------------------------------------------
// ensure_attached — core idempotent attachment function
// ---------------------------------------------------------------------------

fn ensure_attached(render_hwnd: HWND) -> Result<HWND> {
    // Step 1: Locate Progman.
    let progman = unsafe { FindWindowW(w!("Progman"), None) }?;
    if progman.0.is_null() {
        eprintln!("  [!] Progman not found — Explorer may not be running");
        return Err(Error::from_thread());
    }
    println!("  Progman : {:?}", progman.0);

    // Step 2: Send 0x052C to Progman (idempotent).
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

    // Step 3: Find the target WorkerW with retry (~2s timeout).
    let workerw = match find_workerw_retry() {
        Some(hwnd) => hwnd,
        None => {
            eprintln!("  [!] Target WorkerW not found after 2s timeout");
            return Err(Error::new(HRESULT(0), "WorkerW not found after retry"));
        }
    };
    println!("  WorkerW : {:?}", workerw.0);

    // Step 4: SetParent render window into WorkerW.
    unsafe {
        SetParent(render_hwnd, Some(workerw))?;
    }

    // Step 5: Update style and position.
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

/// Poll EnumWindows for the target WorkerW, up to ~2s (8 × 250ms).
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

// ---------------------------------------------------------------------------
// find_workerw_once — single-pass WorkerW discovery via EnumWindows
// ---------------------------------------------------------------------------

/// Single EnumWindows pass. Returns null HWND if not found (no Win32 error).
fn find_workerw_once() -> HWND {
    let mut found = HWND(ptr::null_mut());
    unsafe {
        let _ = EnumWindows(Some(enum_windows_proc), LPARAM(&raw mut found as isize));
    }
    found
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
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

// ---------------------------------------------------------------------------
// Control window WndProc
// ---------------------------------------------------------------------------

unsafe extern "system" fn control_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_id = TASKBAR_MSG_ID.with(Cell::get);

    if msg == taskbar_id {
        println!("\n[!!] TaskbarCreated — Explorer restarted. Recreating render window…");
        let hinstance = process_hinstance();
        match create_and_attach(hinstance) {
            Ok(new_hwnd) => {
                RENDER_HWND_RAW.with(|c| c.set(new_hwnd.0 as isize));
                println!("[✓] Recovery complete.\n");
            }
            Err(e) => {
                eprintln!("[✗] Recovery failed: {}\n", e);
            }
        }
        return LRESULT(0);
    }

    match msg {
        WM_DISPLAYCHANGE => {
            println!("\n[!!] WM_DISPLAYCHANGE — repositioning render window…");
            let rh = render_hwnd();
            if !rh.0.is_null() {
                match ensure_attached(rh) {
                    Ok(_) => println!("[✓] Repositioned.\n"),
                    Err(e) => eprintln!("[✗] Reposition failed: {}\n", e),
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ---------------------------------------------------------------------------
// Render window WndProc
// ---------------------------------------------------------------------------

unsafe extern "system" fn render_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rect);

                    let brush = CreateSolidBrush(COLORREF(0x0000_00FF));
                    if !brush.is_invalid() {
                        FillRect(hdc, &rect, brush);
                    }

                    let _ = EndPaint(hwnd, &ps);
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            println!("[!] Render window destroyed ({:?}).", hwnd.0);
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
