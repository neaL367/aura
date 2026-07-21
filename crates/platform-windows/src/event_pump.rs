use aura_core::playback::PerformanceProfile;

// ---------------------------------------------------------------------------
// HostEvent — events emitted by the Win32 event pump to the daemon
// ---------------------------------------------------------------------------

/// Events that the platform event pump forwards to the daemon orchestrator.
#[derive(Debug, Clone)]
pub enum HostEvent {
    /// Explorer restarted; all host HWNDs are invalid.  Full reattach required.
    ExplorerRestarted,

    /// Display configuration changed (monitor added/removed/moved/resized).
    DisplayChanged,

    /// Power / session state change.
    PerformanceHint(PerformanceProfile),

    /// Graceful shutdown requested (e.g. WM_ENDSESSION, SIGTERM-equivalent).
    ShutdownRequested,
}

// ---------------------------------------------------------------------------
// EventPump (stub — full implementation in Phase 5)
// ---------------------------------------------------------------------------

/// Drives the Win32 message loop on a dedicated thread.
///
/// Emits `HostEvent`s over a `crossbeam_channel` to the daemon orchestrator.
///
/// # Implementation note
/// The full implementation registers for:
/// - `TaskbarCreated` → `ExplorerRestarted`
/// - `WM_DISPLAYCHANGE` → `DisplayChanged`
/// - `WM_POWERBROADCAST` / `WM_WTSSESSION_CHANGE` → `PerformanceHint`
/// - `WM_ENDSESSION` → `ShutdownRequested`
pub struct EventPump {
    sender: crossbeam_channel::Sender<HostEvent>,
    pub receiver: crossbeam_channel::Receiver<HostEvent>,
}

impl EventPump {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }

    /// Spawn the event pump thread.
    ///
    /// The thread runs a Win32 message loop and sends `HostEvent`s.
    pub fn spawn(self) -> std::thread::JoinHandle<()> {
        let _sender = self.sender.clone();
        std::thread::Builder::new()
            .name("aura-event-pump".into())
            .spawn(move || {
                // TODO: Full Win32 message loop implementation in Phase 5.
                // For now, the thread exits immediately.
                tracing::warn!("EventPump: stub implementation — message loop not yet running");
            })
            .expect("failed to spawn event pump thread")
    }
}

impl Default for EventPump {
    fn default() -> Self {
        Self::new()
    }
}
