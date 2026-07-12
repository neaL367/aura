use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE, HANDLE};
use windows::Win32::UI::WindowsAndMessaging::{
    RegisterClassExW, CreateWindowExW, DefWindowProcW, DestroyWindow,
    WNDCLASSEXW, CS_HREDRAW, CS_VREDRAW,
    WS_CHILD, WS_VISIBLE, WS_EX_TOOLWINDOW, WM_DESTROY,
    SystemParametersInfoW, SPI_SETDESKWALLPAPER, SPIF_UPDATEINIFILE, SPIF_SENDCHANGE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};
use windows::Win32::System::StationsAndDesktops::{
    GetProcessWindowStation, GetUserObjectInformationW, UOI_FLAGS, USEROBJECTFLAGS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use tracing::{info, debug};
use crate::utils::error::Result;

pub struct WallpaperWindow {
    pub hwnd: HWND,
}

impl WallpaperWindow {
    /// Creates a visible child window attached to the specified parent HWND.
    pub fn create(parent: HWND, x: i32, y: i32, width: i32, height: i32) -> Result<Self> {
        let hinstance_raw = unsafe { GetModuleHandleW(None)? };
        let hinstance = HINSTANCE(hinstance_raw.0);
        let class_name = w!("AuraWallpaperWindow");

        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
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

        // Register class (registers only once per process, subsequent calls are fine or return error)
        unsafe {
            RegisterClassExW(&wnd_class);
        }

        // Create the child window attached to WorkerW parent
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class_name,
                w!("Aura Wallpaper View"),
                WS_CHILD | WS_VISIBLE,
                x,
                y,
                width,
                height,
                parent,
                None,
                hinstance,
                None,
            )?
        };

        info!("Created wallpaper window: {:?} with parent: {:?}", hwnd, parent);
        Ok(Self { hwnd })
    }

    /// Creates a standalone borderless topmost window (useful for headless testing or custom overlay mode).
    pub fn create_standalone(width: i32, height: i32) -> Result<Self> {
        Self::create_standalone_at(0, 0, width, height)
    }

    /// Creates a standalone borderless topmost window at specific coordinates (useful for multi-monitor headless testing).
    pub fn create_standalone_at(x: i32, y: i32, width: i32, height: i32) -> Result<Self> {
        let hinstance_raw = unsafe { GetModuleHandleW(None)? };
        let hinstance = HINSTANCE(hinstance_raw.0);
        let class_name = w!("AuraWallpaperWindow");

        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
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

        let hwnd = unsafe {
            CreateWindowExW(
                windows::Win32::UI::WindowsAndMessaging::WS_EX_TOPMOST | windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW,
                class_name,
                w!("Aura Wallpaper View"),
                windows::Win32::UI::WindowsAndMessaging::WS_POPUP | windows::Win32::UI::WindowsAndMessaging::WS_VISIBLE,
                x,
                y,
                width,
                height,
                HWND::default(),
                None,
                hinstance,
                None,
            )?
        };

        info!("Created standalone wallpaper window: {:?} at {},{} with size {}x{}", hwnd, x, y, width, height);
        Ok(Self { hwnd })
    }
}

impl Drop for WallpaperWindow {
    fn drop(&mut self) {
        if self.hwnd != HWND::default() {
            debug!("Destroying wallpaper window: {:?}", self.hwnd);
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
            debug!("Wallpaper window WM_DESTROY triggered for HWND: {:?}", hwnd);
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Checks if the current process is running in an interactive window station (visible desktop).
pub fn is_interactive_window_station() -> bool {
    unsafe {
        let hwinsta = match GetProcessWindowStation() {
            Ok(h) => h,
            Err(_) => return false,
        };
        if hwinsta.0.is_null() {
            return false;
        }
        let mut flags = USEROBJECTFLAGS::default();
        let mut len = 0;
        let ok = GetUserObjectInformationW(
            HANDLE(hwinsta.0),
            UOI_FLAGS,
            Some(&mut flags as *mut _ as *mut _),
            std::mem::size_of::<USEROBJECTFLAGS>() as u32,
            Some(&mut len),
        );
        if ok.is_ok() {
            (flags.dwFlags & 1) != 0 // WSF_VISIBLE is 1
        } else {
            false
        }
    }
}

/// Sets the native Windows desktop wallpaper using SystemParametersInfoW.
pub fn set_desktop_wallpaper_native(path: &std::path::Path) -> Result<()> {
    let path_wide = to_wide(path);
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(path_wide.as_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(SPIF_UPDATEINIFILE.0 | SPIF_SENDCHANGE.0),
        )?;
    }
    Ok(())
}

fn to_wide(path: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}
