#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[cfg(target_os = "windows")]
use crossbeam_channel::Sender;

const WM_TRAY_CALLBACK: u32 = 0x8000;
const ID_TRAY_SHOW: u16 = 1000;
const ID_TRAY_QUIT: u16 = 1001;

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    },
    UI::WindowsAndMessaging::{
        AppendMenuW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
        DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos,
        GetMessageW, GetWindowThreadProcessId, IDC_ARROW, IDI_APPLICATION, IsWindowVisible,
        LoadCursorW, LoadIconW, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
        SW_HIDE, SW_RESTORE, SetForegroundWindow, ShowWindow, TPM_RIGHTBUTTON, TrackPopupMenu,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_DESTROY, WM_LBUTTONDOWN, WM_NULL,
        WM_RBUTTONDOWN, WNDCLASSW,
    },
};
#[cfg(target_os = "windows")]
use windows::core::w;

#[cfg(target_os = "windows")]
thread_local! {
    static TRAY_SENDER: std::cell::Cell<Option<Sender<()>>> = const { std::cell::Cell::new(None) };
}

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
    pub fn spawn(self, quit_tx: Sender<()>) -> std::thread::JoinHandle<()> {
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || self.run_message_loop(quit_tx))
            .expect("failed to spawn tray thread")
    }

    #[cfg(not(target_os = "windows"))]
    pub fn spawn(self, _quit_tx: Sender<()>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(|| {})
    }

    #[cfg(target_os = "windows")]
    fn run_message_loop(&self, quit_tx: Sender<()>) {
        TRAY_SENDER.with(|s| s.set(Some(quit_tx)));

        let instance = unsafe { GetModuleHandleW(None).unwrap_or_default() };

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(Self::wnd_proc),
            hInstance: instance.into(),
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
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
            WM_TRAY_CALLBACK => match lparam.0 as u32 {
                WM_LBUTTONDOWN => {
                    Self::toggle_eframe_window();
                    LRESULT(0)
                }
                WM_RBUTTONDOWN => {
                    Self::show_context_menu(hwnd);
                    LRESULT(0)
                }
                _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            },
            WM_COMMAND => {
                let cmd = wparam.0 as u16;
                match cmd {
                    ID_TRAY_SHOW => {
                        Self::toggle_eframe_window();
                        LRESULT(0)
                    }
                    ID_TRAY_QUIT => {
                        // Close the eframe window so the main thread unblocks.
                        let title = windows::core::HSTRING::from(aura_core::WINDOW_TITLE);
                        let _closed = unsafe { FindWindowW(None, &title) }
                            .ok()
                            .filter(|h| !h.is_invalid())
                            .is_some_and(|hwnd| {
                                unsafe {
                                    let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                                        Some(hwnd),
                                        windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                                        WPARAM(0),
                                        LPARAM(0),
                                    );
                                }
                                true
                            });
                        TRAY_SENDER.with(|s| {
                            if let Some(tx) = s.take() {
                                let _ = tx.send(());
                            }
                        });
                        unsafe {
                            PostQuitMessage(0);
                        }
                        LRESULT(0)
                    }
                    _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
                }
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    #[cfg(target_os = "windows")]
    fn toggle_eframe_window() {
        unsafe {
            let title = windows::core::HSTRING::from(aura_core::WINDOW_TITLE);
            let Ok(hwnd) = FindWindowW(None, &title) else {
                return;
            };
            if hwnd.is_invalid() {
                return;
            }
            if IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else {
                let fg_thread = GetWindowThreadProcessId(hwnd, None);
                let cur_thread = GetCurrentThreadId();
                if fg_thread != cur_thread {
                    let _ = AttachThreadInput(cur_thread, fg_thread, true);
                    let _ = SetForegroundWindow(hwnd);
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = AttachThreadInput(cur_thread, fg_thread, false);
                } else {
                    let _ = SetForegroundWindow(hwnd);
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn show_context_menu(hwnd: HWND) {
        unsafe {
            let _ = SetForegroundWindow(hwnd);
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);

            let menu = CreatePopupMenu().unwrap_or_default();
            if menu.is_invalid() {
                return;
            }

            let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_SHOW as usize, w!("Show/Hide Aura"));
            let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_QUIT as usize, w!("Quit"));

            let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, Some(0), hwnd, None);

            // Workaround for MSDN Q135788: send WM_NULL to dismiss the menu
            // when the user clicks outside of it.
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));

            let _ = DestroyMenu(menu);
        }
    }

    #[cfg(target_os = "windows")]
    fn add_icon(hwnd: HWND) {
        let icon = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() };
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY_CALLBACK;
        nid.hIcon = icon;
        let tip: Vec<u16> = format!("{}\0", aura_core::WINDOW_TITLE)
            .encode_utf16()
            .collect();
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
