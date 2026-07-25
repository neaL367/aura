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
            match WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) {
                Ok(_) => {
                    self.session_registered = true;
                    tracing::info!(
                        "Registered for Win32 session notifications (WTSRegisterSessionNotification)"
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to register session notification: {}", e);
                }
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

/// Stateful power and session monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerMonitor {
    is_locked: bool,
    is_display_off: bool,
    on_battery: bool,
}

impl PowerMonitor {
    pub fn new() -> Self {
        Self {
            is_locked: false,
            is_display_off: false,
            on_battery: false,
        }
    }

    /// Process an incoming power or session event, update internal state, and return the current performance profile.
    pub fn handle_event(&mut self, event: PowerEvent) -> PerformanceProfile {
        match event {
            PowerEvent::SessionLocked => self.is_locked = true,
            PowerEvent::SessionUnlocked => self.is_locked = false,
            PowerEvent::DisplayOff => self.is_display_off = true,
            PowerEvent::DisplayOn => self.is_display_off = false,
            PowerEvent::OnBattery => self.on_battery = true,
            PowerEvent::PluggedIn => self.on_battery = false,
        }
        self.current_profile()
    }

    /// Compute the current performance profile based on state hierarchy.
    pub fn current_profile(&self) -> PerformanceProfile {
        if self.is_locked || self.is_display_off {
            PerformanceProfile::Paused
        } else if self.on_battery {
            PerformanceProfile::Balanced
        } else {
            PerformanceProfile::Maximum
        }
    }

    /// Map a raw power event statelessly (starting from default state).
    pub fn profile_for_event(event: PowerEvent) -> PerformanceProfile {
        let mut monitor = Self::new();
        monitor.handle_event(event)
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
