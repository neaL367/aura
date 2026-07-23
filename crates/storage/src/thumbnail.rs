use aura_core::wallpaper::{MediaKind, WallpaperMeta};
use aura_media::MediaDecoder;
use std::path::PathBuf;

/// Manages lazy thumbnail generation and caching under `%APPDATA%/aura/thumbs/`.
pub struct ThumbnailStore;

impl ThumbnailStore {
    /// Directory where wallpaper thumbnails are cached (`%APPDATA%/aura/thumbs`).
    pub fn thumbs_dir() -> PathBuf {
        crate::config_store::ConfigStore::default_path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("thumbs")
    }

    /// Return existing thumbnail path if present, or generate and cache a new thumbnail.
    pub fn get_or_create(meta: &WallpaperMeta) -> Option<PathBuf> {
        let dir = Self::thumbs_dir();
        let target_file = dir.join(format!("{}.jpg", meta.id));

        if target_file.exists() {
            return Some(target_file);
        }

        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!("Failed to create thumbs directory {:?}: {}", dir, e);
            return None;
        }

        let img = match meta.kind {
            MediaKind::Image | MediaKind::Gif => image::open(&meta.path).ok(),
            MediaKind::Video => {
                #[cfg(target_os = "windows")]
                {
                    if let Ok(mut decoder) = aura_platform_windows::MfVideoDecoder::open(&meta.path) {
                        if let Ok(Some(frame)) = decoder.next_frame() {
                            image::RgbaImage::from_raw(frame.width, frame.height, frame.data)
                                .map(image::DynamicImage::ImageRgba8)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    None
                }
            }
        }?;

        let thumb = img.thumbnail(320, 180);
        drop(img);
        let tmp_file = dir.join(format!("{}.tmp", meta.id));

        if let Ok(mut file) = std::fs::File::create(&tmp_file) {
            if thumb.write_to(&mut file, image::ImageFormat::Jpeg).is_ok() {
                let _ = std::fs::remove_file(&target_file);
                if std::fs::rename(&tmp_file, &target_file).is_ok() {
                    tracing::info!(
                        "Generated thumbnail for {:?} at {:?}",
                        meta.path,
                        target_file
                    );
                    return Some(target_file);
                }
            }
            let _ = std::fs::remove_file(&tmp_file);
        }

        None
    }
}
