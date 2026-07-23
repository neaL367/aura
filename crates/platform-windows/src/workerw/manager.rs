use std::ptr;
use windows::Win32::Foundation::HWND;

use super::attachment::attach_to_workerw;
use super::discovery::{find_and_prepare_workerw, find_workerw_once};
use crate::error::PlatformError;

/// Manages the WorkerW attachment lifecycle for a set of host windows.
pub struct WorkerWManager {
    /// Currently known WorkerW HWND. May be null if not yet attached.
    current_workerw: HWND,
}

impl WorkerWManager {
    pub fn new() -> Self {
        Self {
            current_workerw: HWND(ptr::null_mut()),
        }
    }

    /// Find and prepare the WorkerW window handle.
    pub fn find_workerw(&mut self) -> std::result::Result<HWND, PlatformError> {
        let workerw = find_and_prepare_workerw()?;
        self.current_workerw = workerw;
        Ok(workerw)
    }

    /// Find the WorkerW and attach `host_hwnd` to it.
    ///
    /// Idempotent — safe to call repeatedly.
    pub fn ensure_attached(&mut self, host_hwnd: HWND) -> std::result::Result<(), PlatformError> {
        let workerw = self.find_workerw()?;
        attach_to_workerw(host_hwnd, workerw)?;
        Ok(())
    }

    /// Try a single WorkerW discovery pass (no retry). Returns true if attached.
    pub fn try_find_workerw(&mut self) -> bool {
        match find_workerw_once() {
            Ok(workerw) => {
                self.current_workerw = workerw;
                true
            }
            Err(_) => false,
        }
    }

    /// Current WorkerW HWND (null if `ensure_attached` was never called or failed).
    pub fn workerw(&self) -> HWND {
        self.current_workerw
    }
}

impl Default for WorkerWManager {
    fn default() -> Self {
        Self::new()
    }
}
