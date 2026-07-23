#[cfg(target_os = "windows")]
mod windows_tests {
    use aura_core::playback::PerformanceProfile;
    use aura_platform_windows::power::{PowerEvent, PowerMonitor};

    #[test]
    fn session_locked_is_paused() {
        assert_eq!(
            PowerMonitor::profile_for_event(PowerEvent::SessionLocked),
            PerformanceProfile::Paused,
        );
    }

    #[test]
    fn display_off_is_paused() {
        assert_eq!(
            PowerMonitor::profile_for_event(PowerEvent::DisplayOff),
            PerformanceProfile::Paused,
        );
    }

    #[test]
    fn on_battery_is_balanced() {
        assert_eq!(
            PowerMonitor::profile_for_event(PowerEvent::OnBattery),
            PerformanceProfile::Balanced,
        );
    }

    #[test]
    fn plugged_in_is_maximum() {
        assert_eq!(
            PowerMonitor::profile_for_event(PowerEvent::PluggedIn),
            PerformanceProfile::Maximum,
        );
    }

    #[test]
    fn session_unlocked_is_maximum() {
        assert_eq!(
            PowerMonitor::profile_for_event(PowerEvent::SessionUnlocked),
            PerformanceProfile::Maximum,
        );
    }

    #[test]
    fn display_on_is_maximum() {
        assert_eq!(
            PowerMonitor::profile_for_event(PowerEvent::DisplayOn),
            PerformanceProfile::Maximum,
        );
    }
}
