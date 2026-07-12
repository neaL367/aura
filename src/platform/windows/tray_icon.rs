use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{LoadIconW, IDI_APPLICATION};

use crate::utils::error::{AppError, Result};

pub const WM_TRAYICON: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;

pub struct TrayIcon {
    hwnd: HWND,
    id: u32,
}

impl TrayIcon {
    pub fn create(hwnd: HWND, tooltip: &str) -> Result<Self> {
        let nid = build_nid(hwnd, 1, tooltip)?;

        let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
        if !ok.as_bool() {
            return Err(AppError::Platform(
                "Shell_NotifyIconW(NIM_ADD) failed".into(),
            ));
        }

        Ok(Self { hwnd, id: 1 })
    }

    pub fn update_tooltip(&self, tooltip: &str) -> Result<()> {
        let nid = build_nid(self.hwnd, self.id, tooltip)?;
        let ok = unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
        if !ok.as_bool() {
            return Err(AppError::Platform(
                "Shell_NotifyIconW(NIM_MODIFY) failed".into(),
            ));
        }
        Ok(())
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = self.id;
        let ok = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
        if !ok.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_DELETE) failed during TrayIcon drop");
        }
    }
}

fn build_nid(hwnd: HWND, id: u32, tooltip: &str) -> Result<NOTIFYICONDATAW> {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = id;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;

    nid.hIcon = unsafe {
        LoadIconW(None, IDI_APPLICATION)
            .map_err(|e| AppError::Platform(format!("LoadIconW failed: {e}")))?
    };

    let tip_wide = to_wide_fixed::<128>(tooltip);
    nid.szTip = tip_wide;

    Ok(nid)
}

fn to_wide_fixed<const N: usize>(s: &str) -> [u16; N] {
    let mut buf = [0u16; N];
    for (i, unit) in s.encode_utf16().take(N - 1).enumerate() {
        buf[i] = unit;
    }
    buf
}
