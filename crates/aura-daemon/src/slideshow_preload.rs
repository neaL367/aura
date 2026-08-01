use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aura_core::monitor::MonitorId;
use aura_core::wallpaper::{MediaKind, WallpaperId, detect_media_kind};
use aura_media::{DecodedFrame, ImageDecoder, MediaDecoder};

/// Pre-decoded static frame cached for one slideshow monitor.
struct CachedFrame {
    id: WallpaperId,
    frame: Arc<DecodedFrame>,
}

/// Background preloader for slideshow static images.
///
/// Decodes the next cycle's static wallpapers on background threads while the
/// current ones are still on screen, so the interval flip presents instantly
/// instead of hitching on an inline decode. Animated items (GIF/Video) are
/// skipped — they decode on their own worker threads anyway.
pub struct SlideshowPreloader {
    cache: Arc<Mutex<HashMap<MonitorId, CachedFrame>>>,
    handles: Vec<std::thread::JoinHandle<()>>,
}

impl SlideshowPreloader {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            handles: Vec::new(),
        }
    }

    /// Decode the given static images on background threads and cache them.
    ///
    /// The selection was computed from a cloned `SlideshowState`, so a cache
    /// entry may end up unused if the real cycle shuffles differently at the
    /// next fire — that is harmless, the entry is simply replaced/evicted.
    pub fn schedule_next(
        &mut self,
        next: &[(MonitorId, WallpaperId)],
        items: &[aura_core::wallpaper::WallpaperMeta],
    ) {
        // Prune finished preload threads so handles don't accumulate.
        self.handles.retain(|h| !h.is_finished());

        let cache = self.cache.clone();
        for (monitor, id) in next {
            let Some(meta) = items.iter().find(|i| i.id == *id) else {
                continue;
            };
            if detect_media_kind(&meta.path) != Some(MediaKind::Image) {
                continue;
            }
            let path = meta.path.clone();
            let cache = cache.clone();
            let monitor = *monitor;
            let id = *id;
            if let Ok(handle) = std::thread::Builder::new()
                .name("slideshow-preload".into())
                .spawn(move || {
                    let mut decoder = match ImageDecoder::open(&path) {
                        Ok(d) => d,
                        Err(e) => {
                            tracing::warn!(
                                "Slideshow preload failed to open {}: {}",
                                aura_security::redact_path(&path),
                                e
                            );
                            return;
                        }
                    };
                    let Ok(Some(frame)) = decoder.next_frame() else {
                        return;
                    };
                    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
                    guard.insert(
                        monitor,
                        CachedFrame {
                            id,
                            frame: Arc::new(frame),
                        },
                    );
                })
            {
                self.handles.push(handle);
            }
        }
    }

    /// Take the cached frame for `monitor` if it matches the selected `id`.
    /// Consumes the entry so a stale preload is never reused twice.
    pub fn take(&self, monitor: &MonitorId, id: WallpaperId) -> Option<Arc<DecodedFrame>> {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        match guard.get(monitor) {
            Some(c) if c.id == id => guard.remove(monitor).map(|c| c.frame),
            _ => None,
        }
    }

    /// Join all preload threads (deterministic shutdown).
    pub fn join_all(&mut self) {
        for handle in self.handles.drain(..) {
            if let Err(e) = handle.join() {
                tracing::error!("Slideshow preload thread panicked: {:?}", e);
            }
        }
    }
}

impl Default for SlideshowPreloader {
    fn default() -> Self {
        Self::new()
    }
}
