use aura_core::wallpaper::{MediaKind, WallpaperMeta};
use aura_media::MediaDecoder;
use std::path::{Path, PathBuf};

use crate::error::StorageError;

/// Manages lazy thumbnail generation and caching under `%APPDATA%/aura/thumbs/`.
pub struct ThumbnailStore;

impl ThumbnailStore {
    /// Directory where wallpaper thumbnails are cached (`%APPDATA%/aura/thumbs`).
    pub fn thumbs_dir() -> PathBuf {
        crate::config_store::ConfigStore::default_path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("thumbs")
    }

    /// Return existing thumbnail path if present, or generate and cache a new thumbnail.
    pub fn get_or_create(meta: &WallpaperMeta) -> Option<PathBuf> {
        Self::get_or_create_in(meta, &Self::thumbs_dir())
    }

    /// Return existing thumbnail path if present, or generate and cache a new thumbnail in a specific directory.
    pub fn get_or_create_in(meta: &WallpaperMeta, dir: &Path) -> Option<PathBuf> {
        Self::generate_in(meta, dir).ok()
    }

    /// Internal typed thumbnail generation returning a `Result<PathBuf, StorageError>`.
    pub fn generate_in(meta: &WallpaperMeta, dir: &Path) -> Result<PathBuf, StorageError> {
        let target_file = dir.join(format!("{}.jpg", meta.id));

        if target_file.exists() {
            return Ok(target_file);
        }

        std::fs::create_dir_all(dir)?;

        let img = match meta.kind {
            MediaKind::Image | MediaKind::Gif => match image::open(&meta.path) {
                Ok(img) => Ok(img),
                Err(e) => {
                    tracing::warn!("Failed to open image for thumbnail {:?}: {}", meta.path, e);
                    Err(StorageError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to open image for thumbnail: {}", e),
                    )))
                }
            },
            MediaKind::Video => {
                #[cfg(target_os = "windows")]
                {
                    if let Ok(mut decoder) = aura_platform_windows::MfVideoDecoder::open(&meta.path)
                    {
                        if let Ok(Some(frame)) = decoder.next_frame() {
                            image::RgbaImage::from_raw(frame.width, frame.height, frame.data)
                                .map(image::DynamicImage::ImageRgba8)
                                .ok_or_else(|| {
                                    StorageError::Io(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "Invalid RGBA video frame buffer",
                                    ))
                                })
                        } else {
                            Err(StorageError::Io(std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                "Video decoder produced no initial frame",
                            )))
                        }
                    } else {
                        Err(StorageError::Io(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "Failed to initialize Media Foundation video decoder",
                        )))
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Err(StorageError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Video thumbnail generation unsupported on non-Windows",
                    )))
                }
            }
        }?;

        let thumb = img.thumbnail(320, 180);
        drop(img);

        let mut buf = Vec::new();
        thumb
            .write_to(
                &mut std::io::Cursor::new(&mut buf),
                image::ImageFormat::Jpeg,
            )
            .map_err(|e| {
                StorageError::Io(std::io::Error::other(format!("JPEG encode error: {}", e)))
            })?;

        crate::atomic::atomic_save_bytes(&target_file, &buf)?;
        tracing::info!(
            "Generated thumbnail for {:?} at {:?}",
            meta.path,
            target_file
        );
        Ok(target_file)
    }
}
