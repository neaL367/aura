use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_full::{
    DebounceEventResult, Debouncer, FileIdMap, new_debouncer, notify::RecursiveMode,
};

/// Debounced filesystem watcher for automatically monitoring library directories.
pub struct LibraryWatcher {
    _debouncer: Debouncer<notify_debouncer_full::notify::RecommendedWatcher, FileIdMap>,
    watched: Vec<PathBuf>,
}

impl LibraryWatcher {
    /// Create a new `LibraryWatcher` monitoring `paths` with a 500ms debounce quiet period.
    ///
    /// When files are created, modified, or deleted within any watch path, `on_change` callback is invoked.
    pub fn new<F>(paths: &[PathBuf], mut on_change: F) -> Result<Self, String>
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
            watched: Vec::new(),
        };

        for path in paths {
            watcher.add_path(path);
        }

        Ok(watcher)
    }

    /// Add a path to the active filesystem watcher.
    pub fn add_path(&mut self, path: &Path) {
        if !self.watched.contains(&path.to_path_buf()) && path.exists() {
            if let Err(e) = self._debouncer.watch(path, RecursiveMode::Recursive) {
                tracing::warn!("Failed to watch path {}: {}", path.display(), e);
            } else {
                self.watched.push(path.to_path_buf());
                tracing::info!("Filesystem watcher monitoring {}", path.display());
            }
        }
    }

    /// Remove a path from the active filesystem watcher.
    pub fn remove_path(&mut self, path: &Path) {
        if let Err(e) = self._debouncer.unwatch(path) {
            tracing::warn!("Failed to unwatch path {}: {}", path.display(), e);
        } else {
            self.watched.retain(|p| p != path);
            tracing::info!("Filesystem watcher unmonitored {}", path.display());
        }
    }

    /// Remove all watched paths and add new ones.
    pub fn replace_paths(&mut self, paths: &[PathBuf]) {
        let old = std::mem::take(&mut self.watched);
        for p in &old {
            let _ = self._debouncer.unwatch(p);
        }
        for p in paths {
            self.add_path(p);
        }
    }
}
