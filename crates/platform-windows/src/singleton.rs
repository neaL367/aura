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
    _sd: aura_security::SecurityDescriptor,
}

const MUTEX_NAME: windows::core::PCWSTR = w!("Local\\AuraWallpaperdSingleton");

impl ProcessSingleton {
    /// Attempt to acquire the singleton lock.
    ///
    /// Returns `Err(PlatformError::AlreadyRunning)` if another process holds it.
    pub fn acquire() -> std::result::Result<Self, PlatformError> {
        let sd = aura_security::SecurityDescriptor::for_current_user()
            .map_err(|_| PlatformError::WorkerWNotFound)?;

        let sa = sd.as_raw_security_attributes();

        let mutex = unsafe { CreateMutexW(Some(&sa), false, MUTEX_NAME)? };

        let last_error = unsafe { windows::Win32::Foundation::GetLastError() };
        if last_error == windows::Win32::Foundation::ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(mutex);
            }
            return Err(PlatformError::AlreadyRunning);
        }

        Ok(Self { mutex, _sd: sd })
    }
}

impl Drop for ProcessSingleton {
    fn drop(&mut self) {
        if !self.mutex.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.mutex);
            }
        }
    }
}

unsafe impl Send for ProcessSingleton {}
