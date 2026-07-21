use std::ptr;

use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED},
        System::Threading::{CreateMutexW, MUTEX_ALL_ACCESS, ReleaseMutex, WaitForSingleObject},
    },
    core::{Error, Result, w},
};

use crate::error::PlatformError;

/// Named-mutex process singleton.
///
/// Ensures only one `wallpaperd` instance runs at a time.
/// Drop releases the mutex.
pub struct ProcessSingleton {
    mutex: HANDLE,
}

const MUTEX_NAME: windows::core::PCWSTR = w!("Global\\AuraWallpaperdSingleton");

impl ProcessSingleton {
    /// Attempt to acquire the singleton lock.
    ///
    /// Returns `Err(PlatformError::AlreadyRunning)` if another process holds it.
    pub fn acquire() -> std::result::Result<Self, PlatformError> {
        // SAFETY: CreateMutexW with a valid name; initial owner = false.
        let mutex = unsafe { CreateMutexW(None, false, MUTEX_NAME)? };

        // A handle returned but the mutex already exists — check if we own it.
        let last_error = unsafe { windows::Win32::Foundation::GetLastError() };
        if last_error == windows::Win32::Foundation::ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(mutex);
            }
            return Err(PlatformError::AlreadyRunning);
        }

        Ok(Self { mutex })
    }
}

impl Drop for ProcessSingleton {
    fn drop(&mut self) {
        if !self.mutex.is_invalid() {
            // SAFETY: Valid owned handle.
            unsafe {
                let _ = ReleaseMutex(self.mutex);
                let _ = CloseHandle(self.mutex);
            }
        }
    }
}

// SAFETY: The HANDLE is only used from the thread that created the singleton.
// In practice the singleton is held for the entire daemon lifetime.
unsafe impl Send for ProcessSingleton {}
unsafe impl Sync for ProcessSingleton {}
