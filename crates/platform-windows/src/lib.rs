//! `aura-platform-windows` — Windows 11 platform integration.
//!
//! # Module responsibilities
//!
//! - `workerw`: WorkerW discovery and `ensure_attached()`.
//! - `host_window`: Per-monitor HWND lifecycle (create, destroy, recreate).
//! - `monitor_enum`: Stable monitor enumeration with device-path IDs.
//! - `singleton`: Named-mutex process singleton.
//! - `event_pump`: Win32 message loop and `HostEvent` enum.
//! - `power`: Power / session change notifications.
//! - `mf_video`: Media Foundation video decoder.

#[cfg(target_os = "windows")]
pub mod error;
#[cfg(target_os = "windows")]
pub mod event_pump;
#[cfg(target_os = "windows")]
pub mod host_window;
#[cfg(target_os = "windows")]
pub mod mf_video;
#[cfg(target_os = "windows")]
pub mod monitor_enum;
#[cfg(target_os = "windows")]
pub mod power;
#[cfg(target_os = "windows")]
pub mod singleton;
#[cfg(target_os = "windows")]
pub mod workerw;

#[cfg(target_os = "windows")]
pub use error::PlatformError;
#[cfg(target_os = "windows")]
pub use mf_video::MfVideoDecoder;

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

/// Returns process RAM memory usage `(working_set_mb, private_bytes_mb)`.
#[cfg(target_os = "windows")]
pub fn get_process_memory_mb() -> (f32, f32) {
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

// Stubs for non-Windows platforms (e.g. Linux CI check/test)
#[cfg(not(target_os = "windows"))]
pub mod stub {
    use thiserror::Error;

    #[derive(Debug, Clone, Error)]
    pub enum PlatformError {
        #[error("Not supported on this platform")]
        NotSupported,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct HWND(pub *mut std::ffi::c_void);

    pub struct HostWindow;
    impl HostWindow {
        pub fn create() -> Result<Self, PlatformError> {
            Err(PlatformError::NotSupported)
        }
        pub fn hwnd(&self) -> HWND {
            HWND(std::ptr::null_mut())
        }
    }

    pub struct WorkerWManager;
    impl WorkerWManager {
        pub fn new() -> Self {
            Self
        }
        pub fn ensure_attached(&mut self, _host_hwnd: HWND) -> Result<(), PlatformError> {
            Err(PlatformError::NotSupported)
        }
        pub fn workerw(&self) -> HWND {
            HWND(std::ptr::null_mut())
        }
    }

    pub struct MonitorEnumerator;
    impl MonitorEnumerator {
        pub fn new() -> Self {
            Self
        }
        pub fn enumerate(&self) -> Result<Vec<aura_core::monitor::MonitorInfo>, PlatformError> {
            Ok(Vec::new())
        }
    }

    pub struct ProcessSingleton;
    impl ProcessSingleton {
        pub fn acquire() -> Result<Self, PlatformError> {
            Ok(Self)
        }
    }

    pub struct PowerManager;
    impl PowerManager {
        pub fn new() -> Self {
            Self
        }
        pub fn register(&self, _hwnd: HWND) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    pub struct PowerMonitor;
    impl PowerMonitor {
        pub fn new() -> Self {
            Self
        }
        pub fn profile_for_event(_event: PowerEvent) -> aura_core::playback::PerformanceProfile {
            aura_core::playback::PerformanceProfile::Balanced
        }
    }
    impl Default for PowerMonitor {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PowerEvent {
        SessionLocked,
        SessionUnlocked,
        DisplayOff,
        DisplayOn,
        OnBattery,
        PluggedIn,
    }

    pub struct MfVideoDecoder;
    impl MfVideoDecoder {
        pub fn open(_path: &std::path::Path) -> Result<Self, aura_media::error::MediaError> {
            Err(aura_media::error::MediaError::Decode(
                "Not supported".into(),
            ))
        }
    }
    impl aura_media::decoder::MediaDecoder for MfVideoDecoder {
        fn next_frame(
            &mut self,
        ) -> Result<Option<aura_media::decoder::DecodedFrame>, aura_media::error::MediaError>
        {
            Ok(None)
        }
        fn dimensions(&self) -> (u32, u32) {
            (0, 0)
        }
        fn duration_ms(&self) -> u64 {
            0
        }
        fn seek(&mut self, _time_ms: u64) -> Result<(), aura_media::error::MediaError> {
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub enum HostEvent {
        ExplorerRestarted,
        DisplayChanged,
        PerformanceHint(aura_core::playback::PerformanceProfile),
        ShutdownRequested,
    }

    pub struct EventPump {
        pub receiver: crossbeam_channel::Receiver<HostEvent>,
    }
    impl EventPump {
        pub fn new() -> Self {
            let (_, receiver) = crossbeam_channel::unbounded();
            Self { receiver }
        }
        pub fn spawn(self) -> std::thread::JoinHandle<()> {
            std::thread::spawn(|| {})
        }
    }
    impl Default for EventPump {
        fn default() -> Self {
            Self::new()
        }
    }
    pub fn enable_dpi_awareness() -> Result<(), PlatformError> {
        Ok(())
    }
    pub fn get_process_memory_mb() -> (f32, f32) {
        (0.0, 0.0)
    }
}
#[cfg(not(target_os = "windows"))]
pub use stub::*;
