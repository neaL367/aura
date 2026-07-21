//! Phase 0 — WorkerW Desktop Integration Proof
//!
//! **Purpose**: Verify the WorkerW attachment mechanism used by `wallpaperd`
//! before introducing Vulkan complexity. This is a throwaway validation tool.
//!
//! **Expected result**: A solid red rectangle appears *behind* the Windows
//! desktop icons, proving the window is correctly embedded in the WorkerW layer.
//!
//! # WorkerW Algorithm
//!
//! Windows uses a layered desktop architecture:
//!
//! ```text
//! Desktop (GetDesktopWindow)
//! ├── Progman   "Program Manager"   — owns the wallpaper bitmap
//! │   └── WorkerW                  — created by 0x052C; hosts SHELLDLL_DefView
//! │       └── SHELLDLL_DefView     — desktop icon renderer
//! └── WorkerW  (empty)             — OUR TARGET (below the icon layer)
//! ```
//!
//! Sending message `0x052C` to Progman causes Explorer to insert a WorkerW
//! layer between the static wallpaper and SHELLDLL_DefView.  We then find the
//! *empty* WorkerW (the one that immediately follows the SHELLDLL_DefView
//! container in Z-order) and `SetParent` our render window into it.
//!
//! # Explorer Restart Recovery
//!
//! When Explorer restarts all WorkerW HWNDs are destroyed, taking any child
//! windows (including our render windows) with them.  Explorer broadcasts
//! `TaskbarCreated` to all top-level windows when it is ready again.
//!
//! Recovery sequence:
//! 1. Control window receives `TaskbarCreated`.
//! 2. A new render window is created.
//! 3. `ensure_attached()` is called — idempotent, safe to call any number of
//!    times in any order.
//!
//! # Design Notes
//!
//! - **Two windows**:
//!   - `control_hwnd` — top-level (no parent), never shown; receives broadcast
//!     messages (`TaskbarCreated`, `WM_DISPLAYCHANGE`).
//!   - `render_hwnd` — embedded in WorkerW via `SetParent`; shows the proof
//!     visual; recreated after each Explorer restart.
//! - **Thread-local state** — Win32 WndProc callbacks run on the thread that
//!   created the window (during `DispatchMessageW`), so thread-local storage is
//!   the correct sharing mechanism here.
//! - **No unsafe globals** — all mutable state is in thread-locals; the raw
//!   pointer in `LPARAM` is documented at each use site.

#![windows_subsystem = "console"]

use std::cell::Cell;
use std::mem;
use std::ptr;

use windows::{
    core::{w, Error, Result, BOOL},
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateSolidBrush, EndPaint, FillRect, InvalidateRect,
            HBRUSH, PAINTSTRUCT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows,
            FindWindowExW, FindWindowW, GetClientRect, GetMessageW, GetSystemMetrics,
            GetWindowLongPtrW, LoadCursorW, MoveWindow, PostQuitMessage,
            RegisterClassExW, RegisterWindowMessageW, SendMessageTimeoutW,
            SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage,
            CS_HREDRAW, CS_VREDRAW, GWL_STYLE, IDC_ARROW, MSG,
            SEND_MESSAGE_TIMEOUT_FLAGS, SM_CXSCREEN, SM_CYSCREEN,
            SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SW_SHOW,
            WINDOW_EX_STYLE, WNDCLASSEXW, WM_DESTROY, WM_DISPLAYCHANGE, WM_PAINT,
            WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP, WS_VISIBLE,
        },
    },
};

// ---------------------------------------------------------------------------
// Window class name literals (compile-time wide strings)
// ---------------------------------------------------------------------------

const CLASS_CONTROL: windows::core::PCWSTR = w!("AuraProof_Control");
const CLASS_RENDER: windows::core::PCWSTR = w!("AuraProof_Render");

// ---------------------------------------------------------------------------
// Thread-local state
//
// Win32 dispatches WndProc callbacks on the same thread that called
// DispatchMessageW, so thread-local storage is the right mechanism here.
// We store raw handle values as `isize` to sidestep const-fn limitations
// in some versions of the `windows` crate.
// ---------------------------------------------------------------------------

thread_local! {
    /// Message ID for "TaskbarCreated" — registered at startup, stable per session.
    static TASKBAR_MSG_ID: Cell<u32> = Cell::new(0);

    /// Current render window HWND (as isize). Replaced after each Explorer restart.
    static RENDER_HWND_RAW: Cell<isize> = Cell::new(0);

    /// HINSTANCE for this process — needed to recreate windows from WndProc.
    static HINSTANCE_RAW: Cell<isize> = Cell::new(0);

    /// Total successful WorkerW attachments (for proof output).
    static ATTACH_COUNT: Cell<u32> = Cell::new(0);
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

fn main() -> Result<()> {
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
            0, 0, 1, 1,
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

/// Create a new render window and attach it to the WorkerW layer.
///
/// This is the operation that must succeed at startup and after each Explorer
/// restart.  The created `HWND` is returned to the caller.
fn create_and_attach(hinstance: HINSTANCE) -> Result<HWND> {
    // Create the render window as a top-level POPUP initially.
    // After finding WorkerW we SetParent it and update the style.
    //
    // SAFETY: CreateWindowExW — all parameters are valid.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_RENDER,
            w!("AuraProof_Render"),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0, 0, 800, 600,
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

/// Attach `render_hwnd` into the correct WorkerW layer.
///
/// Steps:
/// 1. Find `Progman`.
/// 2. Send `0x052C` to `Progman` to trigger WorkerW layer creation (idempotent).
/// 3. Locate the target WorkerW (empty layer below SHELLDLL_DefView).
/// 4. `SetParent` the render window into WorkerW.
/// 5. Update window style (`WS_CHILD`), size to primary monitor.
///
/// Safe to call repeatedly:
/// - At startup.
/// - After `TaskbarCreated` (Explorer restart).
/// - After `WM_DISPLAYCHANGE`.
fn ensure_attached(render_hwnd: HWND) -> Result<HWND> {
    // Step 1: Locate Progman.
    //
    // SAFETY: FindWindowW is always safe; returns null on not-found.
    let progman = unsafe { FindWindowW(w!("Progman"), None) }?;
    if progman.0.is_null() {
        eprintln!("  [!] Progman not found — Explorer may not be running");
        return Err(Error::from_thread());
    }
    println!("  Progman : {:?}", progman.0);

    // Step 2: Send 0x052C to Progman.
    //
    // This undocumented message causes Explorer to insert (or confirm) the
    // WorkerW rendering layer.  Sending it multiple times is harmless.
    //
    // SAFETY: SendMessageTimeoutW with a valid HWND.
    let mut _result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            progman,
            0x052C,
            WPARAM(0),
            LPARAM(0),
            SEND_MESSAGE_TIMEOUT_FLAGS(0), // SMTO_NORMAL = 0
            1000,
            Some(&raw mut _result),
        );
    }

    // Step 3: Find the target WorkerW.
    let workerw = find_target_workerw()?;
    println!("  WorkerW : {:?}", workerw.0);

    // Step 4: SetParent render window into WorkerW.
    //
    // After this call render_hwnd is a child of workerw.  If workerw is later
    // destroyed (Explorer restart), render_hwnd is destroyed with it.
    //
    // SAFETY: SetParent with valid, non-null HWNDs.
    unsafe {
        SetParent(render_hwnd, Some(workerw))?;
    }

    // Step 5: Update style and position.
    //
    // Replace WS_POPUP with WS_CHILD; size to primary monitor screen area.
    //
    // SAFETY: GetWindowLongPtrW / SetWindowLongPtrW with GWL_STYLE.
    unsafe {
        let style = GetWindowLongPtrW(render_hwnd, GWL_STYLE);
        // Clear WS_POPUP (0x80000000), add WS_CHILD (0x40000000) | WS_VISIBLE
        let new_style = (style & !(WS_POPUP.0 as isize))
            | WS_CHILD.0 as isize
            | WS_VISIBLE.0 as isize;
        SetWindowLongPtrW(render_hwnd, GWL_STYLE, new_style);

        // Apply the style change.
        let _ = SetWindowPos(
            render_hwnd,
            None,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        );

        // Cover the primary monitor.
        // Production code uses MonitorFromWindow + GetMonitorInfo for exact bounds.
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        let _ = MoveWindow(render_hwnd, 0, 0, w, h, true);
        let _ = ShowWindow(render_hwnd, SW_SHOW);
        let _ = InvalidateRect(Some(render_hwnd), None, true);
    }

    Ok(workerw)
}

// ---------------------------------------------------------------------------
// find_target_workerw — WorkerW discovery via EnumWindows
// ---------------------------------------------------------------------------

/// Find the WorkerW that should host wallpaper rendering.
///
/// We enumerate all top-level windows.  For each, we check whether
/// `SHELLDLL_DefView` is a direct child.  When found, the WorkerW we want is
/// the one that comes *after* that window in Z-order:
///
/// ```text
/// FindWindowEx(NULL, shelldll_parent, "WorkerW", NULL)
/// ```
///
/// This gives the empty WorkerW layer that was inserted by the `0x052C` message.
fn find_target_workerw() -> Result<HWND> {
    // State shared between this function and the enum callback.
    let mut found = HWND(ptr::null_mut());

    // SAFETY:
    // - `enum_windows_proc` is a valid `WNDENUMPROC`.
    // - `&raw mut found` creates a raw pointer to a stack variable that remains
    //   valid for the entire (synchronous) duration of EnumWindows.
    // - We always return TRUE from the callback to avoid spurious error returns.
    unsafe {
        // Ignore the Result — we use our own found flag instead of the BOOL return.
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&raw mut found as isize),
        );
    }

    if found.0.is_null() {
        eprintln!("  [!] Target WorkerW not found (SHELLDLL_DefView container absent)");
        Err(Error::from_thread())
    } else {
        Ok(found)
    }
}

/// EnumWindows callback: locates the WorkerW target.
///
/// # Safety
///
/// Why unsafe: Required signature for `WNDENUMPROC`.
///
/// Safety invariant: `lparam` is a valid `*mut HWND` pointing to a stack
/// variable in `find_target_workerw`.  EnumWindows is synchronous, so the
/// stack frame is alive for the entire callback lifetime.
///
/// External API contract: Must return `BOOL`; `TRUE` continues enumeration.
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // Check whether `hwnd` contains SHELLDLL_DefView as a direct child.
    //
    // SAFETY: FindWindowExW with valid HWNDs.
    let def_view = unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None) };
    if def_view.is_err() {
        return BOOL::from(true); // not the container — continue enumeration
    }

    // `hwnd` hosts SHELLDLL_DefView.  The WorkerW we want is the next
    // top-level WorkerW after `hwnd` in Z-order.
    //
    // SAFETY: FindWindowExW with valid HWNDs.
    let target = unsafe { FindWindowExW(None, Some(hwnd), w!("WorkerW"), None) };
    if let Ok(target_hwnd) = target {
        // SAFETY: `lparam` is a valid `*mut HWND` per the contract above.
        let slot = unsafe { &mut *(lparam.0 as *mut HWND) };
        *slot = target_hwnd;
        // Continue enumeration (TRUE) rather than returning FALSE, which would
        // propagate as a Win32 error through the EnumWindows Result wrapper.
    }

    BOOL::from(true)
}

// ---------------------------------------------------------------------------
// Control window WndProc
// ---------------------------------------------------------------------------

/// WndProc for the control (message-receiving) window.
///
/// # Safety
///
/// Why unsafe: Required signature for `WNDPROC`.
///
/// Safety invariant: `hwnd` is always a valid HWND while this procedure runs;
/// thread-local state is only read/written from this thread.
unsafe extern "system" fn control_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_id = TASKBAR_MSG_ID.with(Cell::get);

    // TaskbarCreated — Explorer has restarted.
    //
    // The old WorkerW and our render window have been destroyed.
    // Create a new render window and re-attach.
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
        // WM_DISPLAYCHANGE — monitor configuration changed.
        //
        // The WorkerW may have resized or moved.  Re-run ensure_attached to
        // reposition our render window.
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

        // WM_DESTROY — control window is being destroyed (program shutdown).
        WM_DESTROY => {
            // SAFETY: PostQuitMessage is always safe to call.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }

        _ =>
        // SAFETY: DefWindowProcW with valid HWND and message parameters.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ---------------------------------------------------------------------------
// Render window WndProc
// ---------------------------------------------------------------------------

/// WndProc for the render window embedded in WorkerW.
///
/// Paints a solid red rectangle to prove the window is visible behind
/// desktop icons.  In production this is replaced by Vulkan presentation.
///
/// # Safety
///
/// Why unsafe: Required signature for `WNDPROC`.
///
/// Safety invariant: `hwnd` is a valid HWND; BeginPaint/EndPaint are called
/// in a matched pair.
unsafe extern "system" fn render_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: BeginPaint/EndPaint matched pair; hwnd is valid.
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            if !hdc.is_invalid() {
                let mut rect = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rect);

                    // Red fill: COLORREF(0x0000_00FF) = RGB(255, 0, 0).
                    let brush = CreateSolidBrush(COLORREF(0x0000_00FF));
                    if !brush.is_invalid() {
                        FillRect(hdc, &rect, brush);
                        // Note: In this proof tool we intentionally leak the brush
                        // to keep the code simple.  Production code uses a cached
                        // HBRUSH and calls DeleteObject at shutdown.
                    }

                    let _ = EndPaint(hwnd, &ps);
                }
            }
            LRESULT(0)
        }

        // WM_DESTROY — this window was destroyed.
        //
        // This fires when the WorkerW parent is destroyed (Explorer restart).
        // We do NOT call PostQuitMessage here; the control window owns shutdown.
        WM_DESTROY => {
            println!("[!] Render window destroyed ({:?}).", hwnd.0);
            LRESULT(0)
        }

        _ =>
        // SAFETY: DefWindowProcW with valid parameters.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
