pub mod decoder;
pub mod demuxer;

pub use decoder::MfVideoDecoder;
pub use demuxer::MfH264Demuxer;

#[cfg(target_os = "windows")]
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFStartup};

#[cfg(target_os = "windows")]
pub fn ensure_mf_initialized() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
            tracing::error!("Failed to initialize Media Foundation: {}", e);
        } else {
            tracing::info!("Windows Media Foundation initialized successfully");
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_mf_initialized() {}
