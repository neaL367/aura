use crate::decoder::{DecodedFrame, MediaDecoder};
use crate::error::MediaError;
use std::path::Path;

#[cfg(target_os = "windows")]
use windows::Win32::Media::MediaFoundation::{
    IMFSample, IMFSourceReader, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READER_ALL_STREAMS,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MF_VERSION,
    MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Video,
    MFSTARTUP_FULL, MFStartup, MFVideoFormat_RGB32,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
#[cfg(target_os = "windows")]
use windows::core::GUID;

#[cfg(target_os = "windows")]
fn ensure_mf_initialized() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
            tracing::error!("Failed to initialize Media Foundation: {}", e);
        } else {
            tracing::info!("Windows Media Foundation initialized successfully");
        }
    });
}

/// Hardware-accelerated video decoder using Windows Media Foundation (IMFSourceReader).
pub struct MfVideoDecoder {
    #[cfg(target_os = "windows")]
    reader: IMFSourceReader,
    width: u32,
    height: u32,
    #[cfg(target_os = "windows")]
    last_pts_100ns: i64,
}

// SAFETY: IMFSourceReader operations are internally synchronized in Media Foundation.
unsafe impl Send for MfVideoDecoder {}

impl MfVideoDecoder {
    #[cfg(target_os = "windows")]
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        ensure_mf_initialized();

        let path_str = path
            .to_str()
            .ok_or_else(|| MediaError::Decode(format!("Invalid path unicode: {:?}", path)))?;
        let hstring = windows::core::HSTRING::from(path_str);

        unsafe {
            let mut attr = None;
            let _ = MFCreateAttributes(&mut attr, 1);
            if let Some(ref a) = attr {
                let _ = a.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1);
            }

            let reader = MFCreateSourceReaderFromURL(&hstring, attr.as_ref()).map_err(|e| {
                MediaError::Decode(format!(
                    "MFCreateSourceReaderFromURL failed for {:?}: {}",
                    path, e
                ))
            })?;

            let _ = reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
            let _ = reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true);

            let media_type = MFCreateMediaType()
                .map_err(|e| MediaError::Decode(format!("MFCreateMediaType failed: {}", e)))?;

            media_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| MediaError::Decode(e.to_string()))?;
            media_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
                .map_err(|e| MediaError::Decode(e.to_string()))?;

            reader
                .SetCurrentMediaType(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    None,
                    &media_type,
                )
                .map_err(|e| {
                    MediaError::Decode(format!("SetCurrentMediaType RGB32 failed: {}", e))
                })?;

            let current_type = reader
                .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
                .map_err(|e| MediaError::Decode(format!("GetCurrentMediaType failed: {}", e)))?;

            let frame_size = current_type
                .GetUINT64(&MF_MT_FRAME_SIZE)
                .map_err(|e| MediaError::Decode(format!("GetUINT64 frame size failed: {}", e)))?;

            let width = (frame_size >> 32) as u32;
            let height = (frame_size & 0xFFFFFFFF) as u32;

            tracing::info!(
                "MfVideoDecoder initialized for {:?}: {}x{}",
                path,
                width,
                height
            );

            Ok(Self {
                reader,
                width,
                height,
                last_pts_100ns: 0,
            })
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        Err(MediaError::Decode(format!(
            "Media Foundation video decoding is only supported on Windows targets (path: {:?})",
            path
        )))
    }
}

impl MediaDecoder for MfVideoDecoder {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn loop_reset(&mut self) -> Result<(), MediaError> {
        #[cfg(target_os = "windows")]
        unsafe {
            let var = PROPVARIANT::default();
            let null_guid = GUID::default();
            let _ = self.reader.SetCurrentPosition(&null_guid, &var);
            self.last_pts_100ns = 0;
        }
        Ok(())
    }

    fn next_frame(&mut self) -> Result<Option<DecodedFrame>, MediaError> {
        #[cfg(target_os = "windows")]
        unsafe {
            let mut flags: u32 = 0;
            let mut timestamp: i64 = 0;
            let mut sample: Option<IMFSample> = None;

            self.reader
                .ReadSample(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                )
                .map_err(|e| MediaError::Decode(format!("ReadSample failed: {}", e)))?;

            if (flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                self.loop_reset()?;
                return Ok(None);
            }

            let sample = match sample {
                Some(s) => s,
                None => return Ok(None),
            };

            let buffer = sample.ConvertToContiguousBuffer().map_err(|e| {
                MediaError::Decode(format!("ConvertToContiguousBuffer failed: {}", e))
            })?;

            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut max_len: u32 = 0;
            let mut cur_len: u32 = 0;

            buffer
                .Lock(&mut ptr, Some(&mut max_len), Some(&mut cur_len))
                .map_err(|e| MediaError::Decode(format!("Buffer Lock failed: {}", e)))?;

            let mut data = vec![0u8; cur_len as usize];
            std::ptr::copy_nonoverlapping(ptr, data.as_mut_ptr(), cur_len as usize);
            let _ = buffer.Unlock();

            // Swizzle BGRA -> RGBA for Vulkan texture format compatibility
            for chunk in data.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }

            let timestamp_ms = (timestamp / 10_000).max(0) as u64;
            let delta_100ns = if timestamp > self.last_pts_100ns {
                timestamp - self.last_pts_100ns
            } else {
                333_333 // Default ~30fps in 100ns units
            };
            self.last_pts_100ns = timestamp;
            let duration_ms = (delta_100ns / 10_000).max(10) as u64;

            let frame = DecodedFrame {
                width: self.width,
                height: self.height,
                data,
                timestamp_ms,
                duration_ms,
            };
            let _ = frame.validate();

            Ok(Some(frame))
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(None)
        }
    }
}

/// Detect whether a file is a video by inspecting its extension.
pub fn is_video_by_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm")
    )
}
