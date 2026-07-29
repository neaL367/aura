#[cfg(target_os = "windows")]
use crate::error::PlatformError;

#[cfg(target_os = "windows")]
pub fn enable_dpi_awareness() -> Result<(), PlatformError> {
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn enable_dpi_awareness() -> Result<(), stub::PlatformError> {
    Ok(())
}

/// Returns process RAM memory usage `(working_set_mb, private_bytes_mb)`.
#[cfg(target_os = "windows")]
pub fn process_memory_mb() -> (f32, f32) {
    use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let mut pmc = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc as *mut _ as *mut _, pmc.cb)
            .as_bool()
        {
            let working_set_mb = (pmc.WorkingSetSize as f32) / (1024.0 * 1024.0);
            let pagefile_mb = (pmc.PagefileUsage as f32) / (1024.0 * 1024.0);
            (working_set_mb, pagefile_mb)
        } else {
            (0.0, 0.0)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn process_memory_mb() -> (f32, f32) {
    (0.0, 0.0)
}

/// Trim process working set RAM memory pages back to Windows OS.
#[cfg(target_os = "windows")]
pub fn trim_working_set() {
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn trim_working_set() {}

/// Register a Win32 Console Ctrl+C handler that updates an AtomicBool.
#[cfg(target_os = "windows")]
pub fn register_console_ctrl_handler(flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> bool {
    use windows::Win32::System::Console::{CTRL_C_EVENT, SetConsoleCtrlHandler};
    use windows::core::BOOL;

    static FLAG_PTR: std::sync::atomic::AtomicPtr<std::sync::atomic::AtomicBool> =
        std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

    FLAG_PTR.store(
        std::sync::Arc::into_raw(flag) as *mut _,
        std::sync::atomic::Ordering::Relaxed,
    );

    unsafe extern "system" fn ctrl_handler(ctrl_type: u32) -> BOOL {
        if ctrl_type == CTRL_C_EVENT {
            let ptr = FLAG_PTR.load(std::sync::atomic::Ordering::Relaxed);
            if !ptr.is_null() {
                unsafe { (*ptr).store(true, std::sync::atomic::Ordering::Relaxed) };
            }
            BOOL(1)
        } else {
            BOOL(0)
        }
    }

    unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), true).is_ok() }
}

#[cfg(not(target_os = "windows"))]
pub fn register_console_ctrl_handler(_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> bool {
    true
}
