pub mod handlers;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aura_core::monitor::MonitorId;
use aura_ipc::protocol::{MonitorSummary, Request, Response};
use aura_storage::{LibraryWatcher, config_store::ConfigStore, library_store::LibraryStore};
use tracing::info;

use crate::assignment::AssignmentManager;
use crate::render_thread::RenderCommand;
pub use handlers::OrchestratorState;

#[derive(Clone)]
pub struct Orchestrator {
    state: Arc<Mutex<OrchestratorState>>,
    shutdown_tx: crossbeam_channel::Sender<()>,
}

impl Orchestrator {
    pub fn new(
        monitors: Vec<MonitorSummary>,
        wallpaper_txs: HashMap<MonitorId, crossbeam_channel::Sender<RenderCommand>>,
        shutdown_tx: crossbeam_channel::Sender<()>,
    ) -> Self {
        let active_monitors = monitors.len();
        let config_path = ConfigStore::default_path();
        let config_store = ConfigStore::new(&config_path);
        let mut config = config_store.load().unwrap_or_default();
        if config.library.scan_paths.is_empty() {
            config.library.scan_paths = aura_core::config::LibraryConfig::default().scan_paths;
            let _ = config_store.save(&config);
        }

        let library_path = config_path.with_file_name("library.json");
        let library_store = LibraryStore::new(&library_path);
        let library_items = library_store.load().unwrap_or_default();

        let mut assignments = AssignmentManager::new();
        for a in &config.assignments {
            let direct_match = monitors.iter().any(|m| m.id == a.monitor_id);
            if direct_match {
                assignments.assign(a.monitor_id, a.wallpaper_id);
            } else if let Some(fallback_mon) =
                monitors.iter().find(|m| assignments.get(&m.id).is_none())
            {
                info!(
                    "MonitorId {:?} not found in active monitors; matching fallback display {:?}",
                    a.monitor_id, fallback_mon.id
                );
                assignments.assign(fallback_mon.id, a.wallpaper_id);
            }
        }

        info!(
            "Orchestrator initialized — {} wallpaper(s) in cached library, {} monitor(s)",
            library_items.len(),
            monitors.len()
        );

        let orchestrator = Self {
            state: Arc::new(Mutex::new(OrchestratorState {
                is_paused: false,
                assignments,
                active_monitors,
                monitors,
                library_items,
                config_store,
                library_store,
                wallpaper_txs,
                watcher: None,
            })),
            shutdown_tx,
        };

        orchestrator.trigger_auto_refresh();
        orchestrator
    }

    pub fn set_watcher(&self, watcher: LibraryWatcher) {
        if let Ok(mut state) = self.state.lock() {
            state.watcher = Some(watcher);
        }
    }

    pub fn scan_paths(&self) -> Vec<PathBuf> {
        let state = self.state.lock().unwrap();
        let config = state.config_store.load().unwrap_or_default();
        config.library.scan_paths
    }

    pub fn trigger_auto_refresh(&self) {
        let orch = self.clone();
        std::thread::Builder::new()
            .name("library-rescan".into())
            .spawn(move || {
                let _ = orch.handle_request(aura_ipc::protocol::Request::RefreshLibrary);
            })
            .ok();
    }

    pub fn is_paused(&self) -> bool {
        self.state.lock().unwrap().is_paused
    }

    pub fn set_performance_profile(&self, profile: aura_core::playback::PerformanceProfile) {
        if let Ok(state) = self.state.lock() {
            info!(profile = ?profile, "Broadcasting performance profile to render threads");
            for tx in state.wallpaper_txs.values() {
                let _ = tx.send(RenderCommand::SetPerformanceProfile(profile));
            }
        }
    }

    pub fn update_monitors(
        &self,
        monitors: Vec<MonitorSummary>,
        wallpaper_txs: HashMap<MonitorId, crossbeam_channel::Sender<RenderCommand>>,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.active_monitors = monitors.len();
            state.monitors = monitors;
            state.wallpaper_txs = wallpaper_txs;
            info!(
                "Orchestrator monitors updated — now active: {} monitor(s)",
                state.active_monitors
            );
        }
    }

    pub fn handle_request(&self, request: Request) -> Response {
        handlers::handle_request(&self.state, &self.shutdown_tx, request)
    }
}
