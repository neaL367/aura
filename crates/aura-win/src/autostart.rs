/// Register or unregister the current executable for auto-start with Windows.
#[cfg(target_os = "windows")]
pub fn set_autostart(enabled: bool) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
        RegOpenKeyExW, RegSetValueExW,
    };
    use windows::core::w;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            KEY_SET_VALUE,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }

        let result = if enabled {
            let mut path_wide: Vec<u16> = exe.as_os_str().encode_wide().collect();
            path_wide.push(0);
            let data_bytes =
                std::slice::from_raw_parts(path_wide.as_ptr() as *const u8, path_wide.len() * 2);
            RegSetValueExW(hkey, w!("Aura"), Some(0), REG_SZ, Some(data_bytes)).is_ok()
        } else {
            RegDeleteValueW(hkey, w!("Aura")).is_ok()
        };

        let _ = RegCloseKey(hkey);
        result
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_autostart(_enabled: bool) -> bool {
    true
}
