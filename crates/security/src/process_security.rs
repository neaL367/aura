use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Tracks allowed client PIDs for IPC pipe connections.
pub struct ClientValidator {
    allowed_pids: Arc<Mutex<HashSet<u32>>>,
}

impl ClientValidator {
    /// Create a new validator. An empty allowlist permits all clients.
    pub fn new() -> Self {
        Self {
            allowed_pids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Add a PID to the allowlist.
    pub fn allow_pid(&self, pid: u32) {
        self.allowed_pids.lock().unwrap().insert(pid);
    }

    /// Remove a PID from the allowlist.
    pub fn deny_pid(&self, pid: u32) {
        self.allowed_pids.lock().unwrap().remove(&pid);
    }

    /// Check if a PID is allowed. Returns true if the allowlist is empty
    /// (permissive mode) or if the PID is in the allowlist.
    pub fn is_allowed(&self, pid: u32) -> bool {
        let allowed = self.allowed_pids.lock().unwrap();
        allowed.is_empty() || allowed.contains(&pid)
    }

    /// Check if the validator has any explicit entries.
    pub fn has_restrictions(&self) -> bool {
        !self.allowed_pids.lock().unwrap().is_empty()
    }
}

impl Default for ClientValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ClientValidator {
    fn clone(&self) -> Self {
        Self {
            allowed_pids: Arc::clone(&self.allowed_pids),
        }
    }
}

/// Validate a client PID by checking its executable name against the allowlist
/// (`wallpaper-ui.exe`, `wallpaperd.exe`).
pub fn validate_client_pid(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        validate_client_pid_win32(pid)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = pid;
        true
    }
}

#[cfg(target_os = "windows")]
fn validate_client_pid_win32(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::core::PWSTR;

    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return false,
        };

        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(handle);

        if !ok {
            return false;
        }

        let name = String::from_utf16_lossy(&buf[..len as usize]);
        let file_name = std::path::Path::new(&name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        matches!(file_name, "wallpaper-ui.exe" | "wallpaperd.exe")
    }
}
