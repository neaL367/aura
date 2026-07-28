use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Threading::{CreateMutexW, OpenMutexW},
    },
    core::w,
};

use crate::error::PlatformError;

/// Named-mutex process singleton.
///
/// Ensures only one `wallpaperd` instance runs at a time.
/// Creates the mutex with default security, then applies a restrictive
/// DACL to prevent other-user processes from interfering.
/// Closing the handle on Drop releases the named kernel mutex object.
pub struct ProcessSingleton {
    mutex: HANDLE,
    _sd: aura_security::SecurityDescriptor,
}

const MUTEX_NAME: windows::core::PCWSTR = w!("Local\\AuraWallpaperdSingleton");

// Win32 constants for SetSecurityInfo on a mutex kernel object
const SE_KERNEL_OBJECT: u32 = 6;
const DACL_SECURITY_INFORMATION: u32 = 4;
const MUTEX_ALL_ACCESS: u32 = 0x001F_0001;

#[link(name = "advapi32")]
unsafe extern "system" {
    fn SetSecurityInfo(
        handle: HANDLE,
        object_type: u32,
        security_info: u32,
        owner: *const core::ffi::c_void,
        group: *const core::ffi::c_void,
        dacl: *const core::ffi::c_void,
        sacl: *const core::ffi::c_void,
    ) -> u32;
}

impl ProcessSingleton {
    /// Check if another process holds the singleton without acquiring it.
    /// Uses `OpenMutexW` — succeeds if mutex exists, fails with
    /// `ERROR_FILE_NOT_FOUND` if not.
    pub fn is_running() -> bool {
        use windows::Win32::System::Threading::SYNCHRONIZATION_ACCESS_RIGHTS;
        unsafe {
            let mutex_access = SYNCHRONIZATION_ACCESS_RIGHTS(MUTEX_ALL_ACCESS);
            let handle = OpenMutexW(mutex_access, false, MUTEX_NAME);
            if let Ok(h) = handle {
                let _ = CloseHandle(h);
                true
            } else {
                false
            }
        }
    }

    /// Attempt to acquire the singleton lock.
    ///
    /// Returns `Err(PlatformError::AlreadyRunning)` if another process holds it.
    pub fn acquire() -> std::result::Result<Self, PlatformError> {
        let mutex = match unsafe { CreateMutexW(None, false, MUTEX_NAME) } {
            Ok(h) => h,
            Err(_) => {
                // CreateMutexW failed: the mutex might already exist with a
                // restrictive DACL from a prior instance, making it impossible
                // to open even for checking. Treat any creation failure as
                // AlreadyRunning.
                return Err(PlatformError::AlreadyRunning);
            }
        };

        let last_error = unsafe { windows::Win32::Foundation::GetLastError() };
        if last_error == windows::Win32::Foundation::ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(mutex);
            }
            return Err(PlatformError::AlreadyRunning);
        }

        // Replace the default DACL to restrict access to the current user.
        let sd = aura_security::SecurityDescriptor::for_current_user_with_access(MUTEX_ALL_ACCESS)
            .map_err(|_| PlatformError::WorkerWNotFound)?;

        unsafe {
            let _ = SetSecurityInfo(
                mutex,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null(),
                std::ptr::null(),
                sd.acl_ptr() as *const _,
                std::ptr::null(),
            );
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
