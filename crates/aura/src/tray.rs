#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    },
    UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
        DispatchMessageW, GetMessageW, LoadCursorW, LoadIconW, MSG, PostQuitMessage,
        RegisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY, WNDCLASSW,
    },
};
#[cfg(target_os = "windows")]
use windows::core::w;

pub struct TrayManager {
    shutdown_flag: Arc<AtomicBool>,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(target_os = "windows")]
    pub fn spawn(self) -> std::thread::JoinHandle<()> {
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || self.run_message_loop())
            .expect("failed to spawn tray thread")
    }

    #[cfg(not(target_os = "windows"))]
    pub fn spawn(self) -> std::thread::JoinHandle<()> {
        std::thread::spawn(|| {})
    }

    #[cfg(target_os = "windows")]
    fn run_message_loop(&self) {
        let instance = unsafe { GetModuleHandleW(None).unwrap_or_default() };

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(Self::wnd_proc),
            hInstance: instance.into(),
            hCursor: unsafe { LoadCursorW(None, w!("IDC_ARROW")).unwrap_or_default() },
            lpszClassName: w!("AuraTrayWindow"),
            ..Default::default()
        };

        if unsafe { RegisterClassW(&wc) } == 0 {
            tracing::error!("Tray window class registration failed");
            return;
        }

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("AuraTrayWindow"),
                w!("AuraTrayWindow"),
                WINDOW_STYLE::default(),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                Some(instance.into()),
                None,
            )
        };

        let Ok(hwnd) = hwnd else {
            tracing::error!("Tray window creation failed");
            return;
        };

        Self::add_icon(hwnd);

        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = DispatchMessageW(&msg);
            }
        }
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }

    #[cfg(target_os = "windows")]
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DESTROY => {
                Self::remove_icon(hwnd);
                unsafe {
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    #[cfg(target_os = "windows")]
    fn add_icon(hwnd: HWND) {
        let icon = unsafe { LoadIconW(None, w!("IDI_APPLICATION")).unwrap_or_default() };
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = 0x8000;
        nid.hIcon = icon;
        let tip: Vec<u16> = "Aura Wallpaper\0".encode_utf16().collect();
        let len = tip.len().min(127);
        nid.szTip[..len].copy_from_slice(&tip[..len]);

        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
    }

    #[cfg(target_os = "windows")]
    fn remove_icon(hwnd: HWND) {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}
