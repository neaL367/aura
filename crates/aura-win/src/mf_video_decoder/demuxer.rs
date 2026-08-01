use aura_media::error::MediaError;
use std::path::Path;

#[cfg(target_os = "windows")]
use windows::Win32::Media::MediaFoundation::{
    IMFSample, IMFSourceReader, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READER_ALL_STREAMS,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MFCreateAttributes,
    MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Video, MFVideoFormat_H264,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
#[cfg(target_os = "windows")]
use windows::core::GUID;

/// Pure H.264 container demuxer using Media Foundation (IMFSourceReader with MFVideoFormat_H264).
pub struct MfH264Demuxer {
    #[cfg(target_os = "windows")]
    reader: IMFSourceReader,
    width: u32,
    height: u32,
}

unsafe impl Send for MfH264Demuxer {}

impl MfH264Demuxer {
    #[cfg(target_os = "windows")]
    pub fn open(path: &Path) -> Result<Self, MediaError> {
        super::ensure_mf_initialized();

        let path_str = path.to_str().ok_or_else(|| {
            MediaError::Decode(format!(
                "Invalid path unicode: {}",
                aura_security::redact_path(path)
            ))
        })?;
        let hstring = windows::core::HSTRING::from(path_str);

        unsafe {
            let mut attr = None;
            let _ = MFCreateAttributes(&mut attr, 1);
            if let Some(ref a) = attr {
                let _ = a.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1);
            }

            let reader = MFCreateSourceReaderFromURL(&hstring, attr.as_ref()).map_err(|e| {
                MediaError::Decode(format!("MFCreateSourceReaderFromURL failed: {}", e))
            })?;

            let _ = reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
            let _ = reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true);

            let media_type = MFCreateMediaType()
                .map_err(|e| MediaError::Decode(format!("MFCreateMediaType failed: {}", e)))?;

            media_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| MediaError::Decode(e.to_string()))?;
            media_type
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)
                .map_err(|e| MediaError::Decode(e.to_string()))?;

            reader
                .SetCurrentMediaType(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    None,
                    &media_type,
                )
                .map_err(|e| {
                    MediaError::Decode(format!("SetCurrentMediaType H264 failed: {}", e))
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
                "MfH264Demuxer initialized for {}: {}x{}",
                aura_security::redact_path(path),
                width,
                height
            );

            Ok(Self {
                reader,
                width,
                height,
            })
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn open(_path: &Path) -> Result<Self, MediaError> {
        Err(MediaError::Decode(
            "Media Foundation demuxing is only supported on Windows targets (path suppressed)"
                .to_string(),
        ))
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Reset the demuxer to the beginning of the media (for looping).
    pub fn loop_reset(&mut self) -> Result<(), MediaError> {
        #[cfg(target_os = "windows")]
        unsafe {
            let var = PROPVARIANT::default();
            let null_guid = GUID::default();
            self.reader
                .SetCurrentPosition(&null_guid, &var)
                .map_err(|e| MediaError::Decode(format!("SetCurrentPosition failed: {}", e)))?;
        }
        Ok(())
    }

    /// Read next compressed NAL sample and convert to Annex-B (`0x00000001` start code) format.
    pub fn read_next_annex_b_nal(&mut self) -> Result<Option<(Vec<u8>, u64)>, MediaError> {
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

            let mut avcc_data = vec![0u8; cur_len as usize];
            std::ptr::copy_nonoverlapping(ptr, avcc_data.as_mut_ptr(), cur_len as usize);
            let _ = buffer.Unlock();

            let annex_b_nal = crate::h264_parser::avcc_to_annex_b(&avcc_data);
            let pts_ms = (timestamp / 10_000).max(0) as u64;

            Ok(Some((annex_b_nal, pts_ms)))
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(None)
        }
    }
}
