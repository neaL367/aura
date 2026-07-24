use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Threading::CreateMutexW,
    },
    core::w,
};

use crate::error::PlatformError;

/// Named-mutex process singleton.
///
/// Ensures only one `wallpaperd` instance runs at a time.
/// Closing the handle on Drop releases the named kernel mutex object.
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

        // A handle returned but the mutex already exists — check if another process created it.
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
            // SAFETY: Valid owned handle. Closing the handle releases the named mutex kernel object.
            unsafe {
                let _ = CloseHandle(self.mutex);
            }
        }
    }
}

// SAFETY: ProcessSingleton handle can be safely transferred between threads.
unsafe impl Send for ProcessSingleton {}
