use windows::Win32::Foundation::{HWND, LPARAM, WPARAM, POINT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, AppendMenuW, TrackPopupMenu, DestroyMenu, GetCursorPos,
    PostMessageW, MF_STRING, MF_SEPARATOR, TPM_RIGHTALIGN, TPM_BOTTOMALIGN, TPM_RETURNCMD,
    WM_COMMAND, WM_RBUTTONUP, WM_LBUTTONDBLCLK, MENU_ITEM_FLAGS, TRACK_POPUP_MENU_FLAGS,
};
use windows::core::{w, PCWSTR};

// Menu Command IDs
pub const ID_TRAY_SHOW: usize = 2001;
pub const ID_TRAY_PAUSE: usize = 2002;
pub const ID_TRAY_RESUME: usize = 2003;
pub const ID_TRAY_EXIT: usize = 2004;

pub fn handle_tray_message(hwnd: HWND, lparam: LPARAM) {
    let event = lparam.0 as u32;
    if event == WM_RBUTTONUP {
        let mut pos = POINT::default();
        unsafe {
            if GetCursorPos(&mut pos).is_err() {
                return;
            }
            let hmenu = match CreatePopupMenu() {
                Ok(menu) => menu,
                _ => return,
            };

            let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_STRING.0), ID_TRAY_SHOW, w!("Show Aura"));
            let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, PCWSTR::null());
            let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_STRING.0), ID_TRAY_PAUSE, w!("Pause Wallpapers"));
            let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_STRING.0), ID_TRAY_RESUME, w!("Resume Wallpapers"));
            let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_SEPARATOR.0), 0, PCWSTR::null());
            let _ = AppendMenuW(hmenu, MENU_ITEM_FLAGS(MF_STRING.0), ID_TRAY_EXIT, w!("Exit"));

            let cmd = TrackPopupMenu(
                hmenu,
                TRACK_POPUP_MENU_FLAGS(TPM_RIGHTALIGN.0 | TPM_BOTTOMALIGN.0 | TPM_RETURNCMD.0),
                pos.x,
                pos.y,
                None,
                hwnd,
                None,
            );

            let _ = DestroyMenu(hmenu);

            if cmd.0 != 0 {
                let _ = PostMessageW(
                    Some(hwnd),
                    WM_COMMAND,
                    WPARAM(cmd.0 as usize),
                    LPARAM(0),
                );
            }
        }
    } else if event == WM_LBUTTONDBLCLK {
        unsafe {
            let _ = PostMessageW(
                Some(hwnd),
                WM_COMMAND,
                WPARAM(ID_TRAY_SHOW),
                LPARAM(0),
            );
        }
    }
}
