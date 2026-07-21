use aura_core::playback::PerformanceProfile;

/// Power and session monitor (stub — full implementation in Phase 9).
///
/// In the full implementation this registers for:
/// - `WM_POWERBROADCAST` (AC/battery transition, display on/off)
/// - `WTSRegisterSessionNotification` (session lock/unlock)
/// - `RegisterPowerSettingNotification` (monitor on/off)
pub struct PowerMonitor;

impl PowerMonitor {
    pub fn new() -> Self {
        Self
    }

    /// Map a raw power event to a `PerformanceProfile`.
    ///
    /// Called by the event pump when it receives power notifications.
    pub fn profile_for_event(event: PowerEvent) -> PerformanceProfile {
        match event {
            PowerEvent::SessionLocked | PowerEvent::DisplayOff => PerformanceProfile::Paused,
            PowerEvent::OnBattery => PerformanceProfile::Balanced,
            PowerEvent::PluggedIn | PowerEvent::SessionUnlocked | PowerEvent::DisplayOn => {
                PerformanceProfile::Maximum
            }
        }
    }
}

impl Default for PowerMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Raw power/session event from Win32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    SessionLocked,
    SessionUnlocked,
    DisplayOff,
    DisplayOn,
    OnBattery,
    PluggedIn,
}
