use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_full::{
    DebounceEventResult, Debouncer, FileIdMap, new_debouncer, notify::RecursiveMode,
};

/// Debounced filesystem watcher for automatically monitoring library scan directories.
pub struct LibraryWatcher {
    _debouncer: Debouncer<notify_debouncer_full::notify::RecommendedWatcher, FileIdMap>,
}

impl LibraryWatcher {
    /// Create a new `LibraryWatcher` monitoring `scan_paths` with a 500ms debounce quiet period.
    ///
    /// When files are created, modified, or deleted within any scan path, `on_change` callback is invoked.
    pub fn new<F>(scan_paths: &[PathBuf], mut on_change: F) -> Result<Self, String>
    where
        F: FnMut() + Send + 'static,
    {
        let debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    let cache_dir = crate::ThumbnailStore::thumbs_dir();
                    let has_external_event = events
                        .iter()
                        .any(|ev| ev.paths.iter().any(|p| !p.starts_with(&cache_dir)));

                    if has_external_event {
                        tracing::info!(
                            "Filesystem watcher detected event(s) outside cache — triggering auto-refresh"
                        );
                        on_change();
                    }
                }
                Err(errors) => {
                    for err in errors {
                        tracing::warn!("Filesystem watcher error: {:?}", err);
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to create debouncer: {}", e))?;

        let mut watcher = Self {
            _debouncer: debouncer,
        };

        for path in scan_paths {
            watcher.add_path(path);
        }

        Ok(watcher)
    }

    /// Add a path to the active filesystem watcher.
    pub fn add_path(&mut self, path: &Path) {
        if path.exists() {
            if let Err(e) = self._debouncer.watch(path, RecursiveMode::Recursive) {
                tracing::warn!("Failed to watch scan path {}: {}", path.display(), e);
            } else {
                tracing::info!("Filesystem watcher actively monitoring {}", path.display());
            }
        }
    }

    /// Remove a path from the active filesystem watcher.
    pub fn remove_path(&mut self, path: &Path) {
        if let Err(e) = self._debouncer.unwatch(path) {
            tracing::warn!("Failed to unwatch scan path {}: {}", path.display(), e);
        } else {
            tracing::info!("Filesystem watcher unmonitored {}", path.display());
        }
    }
}
