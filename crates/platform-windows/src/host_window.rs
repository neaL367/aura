use std::mem;

use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, IDC_ARROW,
            LoadCursorW, RegisterClassExW, WM_DESTROY, WNDCLASSEXW, WS_CLIPCHILDREN,
            WS_CLIPSIBLINGS, WS_POPUP,
        },
    },
    core::{Error, w},
};

use crate::error::PlatformError;

/// Class name for host windows created by this module.
const HOST_CLASS: windows::core::PCWSTR = w!("AuraHostWindow");
static HOST_CLASS_REGISTERED: std::sync::OnceLock<std::result::Result<(), String>> =
    std::sync::OnceLock::new();

// ---------------------------------------------------------------------------
// HostWindow — a Win32 HWND lifecycle wrapper
// ---------------------------------------------------------------------------

/// A rendering host window embedded in the WorkerW layer.
///
/// One `HostWindow` is created per active monitor.  The HWND is positioned at
/// the monitor's virtual screen coordinates relative to the WorkerW's client area.
///
/// **Ownership**: `HostWindow` owns the HWND.  Dropping it destroys the window.
/// After Explorer restarts (WorkerW destroyed), the HWND becomes invalid; callers
/// must detect this and call `HostWindow::recreate()`.
pub struct HostWindow {
    hwnd: HWND,
    /// Whether the HWND is still valid (set to false on WM_DESTROY).
    valid: bool,
}

impl HostWindow {
    /// Create a new host window using the render-target style.
    ///
    /// - `WS_POPUP` — top-level initially; later reparented into WorkerW.
    /// - `WS_EX_NOREDIRECTIONBITMAP` — skip DWM redirection; required for
    ///   direct GPU presentation (Vulkan surface).
    pub fn create() -> std::result::Result<Self, PlatformError> {
        ensure_class_registered()?;

        let hmodule = unsafe { GetModuleHandleW(None)? };
        let hinstance = HINSTANCE(hmodule.0);

        let w = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
            )
        };
        let h = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
            )
        };

        let hwnd = unsafe {
            CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
                HOST_CLASS,
                w!("AuraHost"),
                WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                0,
                0,
                if w > 0 { w } else { 1920 },
                if h > 0 { h } else { 1080 },
                None,
                None,
                Some(hinstance),
                None,
            )
            .map_err(|_| PlatformError::WindowCreation)?
        };

        Ok(Self { hwnd, valid: true })
    }

    /// Return the raw HWND (for Vulkan surface creation and platform APIs).
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// True if the underlying HWND is still valid.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Mark this window as invalid (called after WorkerW destruction is detected).
    pub fn invalidate(&mut self) {
        self.valid = false;
    }
}

impl Drop for HostWindow {
    fn drop(&mut self) {
        if self.valid && !self.hwnd.0.is_null() {
            // SAFETY: Detach from parent and DestroyWindow on a valid HWND we own.
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::SetParent;
                let _ = SetParent(self.hwnd, None);
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Window class registration (once per process)
// ---------------------------------------------------------------------------

fn ensure_class_registered() -> std::result::Result<(), PlatformError> {
    let res =
        HOST_CLASS_REGISTERED.get_or_init(|| register_class_impl().map_err(|e| e.to_string()));
    res.as_ref()
        .map(|_| ())
        .map_err(|_| PlatformError::WindowCreation)
}

fn register_class_impl() -> std::result::Result<(), PlatformError> {
    let hmodule = unsafe { GetModuleHandleW(None)? };
    let hinstance = HINSTANCE(hmodule.0);
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW)? };

    let wc = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(host_wnd_proc),
        hInstance: hinstance,
        hCursor: cursor,
        lpszClassName: HOST_CLASS,
        ..Default::default()
    };

    if unsafe { RegisterClassExW(&wc) } == 0 {
        return Err(PlatformError::Win32(Error::from_thread()));
    }
    Ok(())
}

/// Minimal WndProc for host windows.
///
/// The Vulkan renderer owns presentation; this proc only handles destruction.
///
/// # Safety
/// Required signature for `WNDPROC`.
unsafe extern "system" fn host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => LRESULT(0), // do not PostQuitMessage from a render window
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
