use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, PostQuitMessage, RegisterClassExW,
    ShowWindow, CW_USEDEFAULT, SW_HIDE, SW_SHOW, WM_CLOSE, WM_DESTROY, WNDCLASSEXW,
    WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW, WM_COMMAND, CS_HREDRAW, CS_VREDRAW,
};
use windows::core::{w, PCWSTR};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use crate::platform::windows::tray_icon::WM_TRAYICON;
use crate::ui::egui_backend::EguiBackend;
use crate::utils::error::{AppError, Result};

const CLASS_NAME: PCWSTR = w!("AuraMainWindow");

/// Stashes the HWND so the free-function wnd_proc can look up state.
pub static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

pub struct MainWindow {
    pub hwnd: HWND,
    pub egui: EguiBackend,
}

impl MainWindow {
    pub fn create(device: &windows::Win32::Graphics::Direct3D11::ID3D11Device) -> Result<Self> {
        let hinstance_raw = unsafe { GetModuleHandleW(None).map_err(|e| AppError::Platform(format!("GetModuleHandleW failed: {e}")))? };
        let hinstance = HINSTANCE(hinstance_raw.0);

        register_class(hinstance)?;

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_APPWINDOW,
                CLASS_NAME,
                w!("Aura"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1280,
                800,
                None,
                None,
                Some(hinstance),
                None,
            )
        }
        .map_err(|e| AppError::Platform(format!("CreateWindowExW (main window) failed: {e}")))?;

        MAIN_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

        let egui = EguiBackend::new(hwnd, device)?;

        Ok(Self { hwnd, egui })
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn destroy(&self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

impl Drop for MainWindow {
    fn drop(&mut self) {
        // Clear the global HWND stash first so any in-flight wnd_proc calls see 0.
        MAIN_HWND.store(0, Ordering::SeqCst);
        // Destroy the Win32 window. This fires WM_DESTROY → PostQuitMessage, but
        // that message is ignored if the pump has already exited.
        //
        // Drop ordering note: Rust struct fields drop in DECLARATION ORDER.
        // CompositionRoot declares tray_icon BEFORE main_window, so TrayIcon::drop()
        // (= Shell_NotifyIconW NIM_DELETE) runs BEFORE this DestroyWindow call,
        // ensuring NIM_DELETE sees a live HWND.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn register_class(hinstance: HINSTANCE) -> Result<()> {
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };

    let atom = unsafe { RegisterClassExW(&wc) };
    if atom == 0 {
        return Err(AppError::Platform(
            "RegisterClassExW (main window class) failed".into(),
        ));
    }
    Ok(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLOSE => {
            // Hide, don't destroy.
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_DESTROY => {
            // Window has been destroyed. PostQuitMessage so the pump exits if it
            // hasn't already (handles the case where the OS destroys the window
            // externally rather than via our Drop or Exit flow).
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_TRAYICON => {
            crate::ui::tray_dispatch::handle_tray_message(hwnd, lparam);
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd_id = wparam.0;
            match cmd_id {
                crate::ui::tray_dispatch::ID_TRAY_SHOW => {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                }
                crate::ui::tray_dispatch::ID_TRAY_EXIT => {
                    // Post WM_QUIT directly — do NOT call DestroyWindow here.
                    // Calling DestroyWindow inside the message pump would invalidate
                    // the HWND before CompositionRoot drops, causing TrayIcon::Drop
                    // to call Shell_NotifyIconW(NIM_DELETE) with a dead handle.
                    // Instead we let the Drop sequence handle destruction:
                    //   TrayIcon::drop() → NIM_DELETE (HWND still live) ✓
                    //   MainWindow::drop() → DestroyWindow (HWND torn down) ✓
                    PostQuitMessage(0);
                }
                // Pause and Resume commands will be handled by the message pump or composition_root
                _ => {}
            }
            LRESULT(0)
        }
        _ => {
            crate::ui::egui_backend::forward_input_message(hwnd, msg, wparam, lparam);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}
