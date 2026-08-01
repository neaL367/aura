use std::path::PathBuf;

use aura_core::wallpaper::WallpaperMeta;
use aura_ipc::protocol::WallpaperEntry;

pub(super) fn build_wallpaper_list(items: &[WallpaperMeta]) -> Vec<WallpaperEntry> {
    let entries: Vec<WallpaperEntry> = items.iter().map(WallpaperEntry::from).collect();
    let n = entries.len();

    // Generate missing thumbnails in parallel with a small worker pool.
    // Handlers run under `spawn_blocking`, so blocking std threads are safe.
    let thumb_paths: Vec<Option<PathBuf>> = if n < 2 {
        items
            .iter()
            .map(aura_storage::ThumbnailStore::get_or_create)
            .collect()
    } else {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let workers = n.min(4);
        let next = AtomicUsize::new(0);
        let metas: Vec<&WallpaperMeta> = items.iter().collect();
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for _ in 0..workers {
                let next_ref = &next;
                let metas_ref = &metas;
                handles.push(scope.spawn(move || {
                    let mut out = Vec::new();
                    loop {
                        let i = next_ref.fetch_add(1, Ordering::Relaxed);
                        if i >= metas_ref.len() {
                            break;
                        }
                        out.push((i, aura_storage::ThumbnailStore::get_or_create(metas_ref[i])));
                    }
                    out
                }));
            }
            let mut results: Vec<Option<PathBuf>> = vec![None; n];
            for handle in handles {
                for (i, thumb) in handle.join().expect("thumbnail worker panicked") {
                    results[i] = thumb;
                }
            }
            results
        })
    };

    let mut list = Vec::with_capacity(n);
    for (mut entry, thumb) in entries.into_iter().zip(thumb_paths) {
        entry.thumbnail_path = thumb;
        list.push(entry);
    }
    list
}
