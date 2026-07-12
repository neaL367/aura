use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE};
use windows::Win32::UI::WindowsAndMessaging::{
    RegisterClassExW, CreateWindowExW, DefWindowProcW, DestroyWindow,
    WNDCLASSEXW, CS_HREDRAW, CS_VREDRAW, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_USER, PostThreadMessageW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use tracing::{info, debug};
use crate::utils::error::Result;

pub struct NotificationWindow {
    pub hwnd: HWND,
}

impl NotificationWindow {
    pub fn create() -> Result<Self> {
        let hinstance_raw = unsafe { GetModuleHandleW(None)? };
        let hinstance = HINSTANCE(hinstance_raw.0);
        let class_name = w!("AuraNotificationWindow");

        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(notification_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: Default::default(),
            hCursor: Default::default(),
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: class_name,
            hIconSm: Default::default(),
        };

        unsafe {
            RegisterClassExW(&wnd_class);
        }

        // Invisible top-level window (WS_POPUP, no WS_VISIBLE)
        let hwnd = unsafe {
            CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
                class_name,
                w!("Aura Notification Receiver"),
                windows::Win32::UI::WindowsAndMessaging::WS_POPUP,
                0,
                0,
                0,
                0,
                Some(HWND::default()),
                None,
                Some(hinstance),
                None,
            )?
        };

        info!("Created hidden notification window: {:?}", hwnd);
        Ok(Self { hwnd })
    }
}

impl Drop for NotificationWindow {
    fn drop(&mut self) {
        if self.hwnd != HWND::default() {
            debug!("Destroying hidden notification window: {:?}", self.hwnd);
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

unsafe extern "system" fn notification_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
            debug!("Notification window WM_DESTROY triggered for HWND: {:?}", hwnd);
        }
        WM_DISPLAYCHANGE => {
            info!("Notification WindowProc: WM_DISPLAYCHANGE received. Posting sync event to main thread...");
            unsafe {
                let main_thread_id = windows::Win32::System::Threading::GetCurrentThreadId();
                let ok = PostThreadMessageW(
                    main_thread_id,
                    WM_USER + 100,
                    WPARAM(0),
                    LPARAM(0),
                );
                if ok.is_err() {
                    tracing::warn!("Notification WindowProc: Failed to post WM_USER+100 thread message (queue not ready).");
                }
            }
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
