use aura_core::playback::PerformanceProfile;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::Power::{
    HPOWERNOTIFY, RegisterPowerSettingNotification, UnregisterPowerSettingNotification,
};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_WINDOW_HANDLE;
use windows::core::GUID;

// GUID_CONSOLE_DISPLAY_STATE = {6fe69556-9f7e-41e0-a985-f73d91117470}
const GUID_CONSOLE_DISPLAY_STATE: GUID = GUID::from_u128(0x6fe69556_9f7e_41e0_a985_f73d91117470);

pub struct PowerManager {
    power_notify_handle: HPOWERNOTIFY,
    session_registered: bool,
}

impl PowerManager {
    pub fn new() -> Self {
        Self {
            power_notify_handle: HPOWERNOTIFY::default(),
            session_registered: false,
        }
    }

    pub fn register(&mut self, hwnd: HWND) {
        unsafe {
            if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION).is_ok() {
                self.session_registered = true;
                tracing::info!(
                    "Registered for Win32 session notifications (WTSRegisterSessionNotification)"
                );
            }
            match RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &GUID_CONSOLE_DISPLAY_STATE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            ) {
                Ok(h) => {
                    self.power_notify_handle = h;
                    tracing::info!(
                        "Registered for Win32 power notifications (GUID_CONSOLE_DISPLAY_STATE)"
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to register power setting notification: {}", e);
                }
            }
        }
    }

    pub fn unregister(&mut self, hwnd: HWND) {
        unsafe {
            if self.power_notify_handle != HPOWERNOTIFY::default() {
                let _ = UnregisterPowerSettingNotification(self.power_notify_handle);
                self.power_notify_handle = HPOWERNOTIFY::default();
            }
            if self.session_registered {
                let _ = WTSUnRegisterSessionNotification(hwnd);
                self.session_registered = false;
            }
        }
    }
}

impl Default for PowerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Power and session monitor.
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
