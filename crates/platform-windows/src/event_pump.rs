use std::sync::{Arc, Mutex};

use aura_core::playback::PerformanceProfile;
use crossbeam_channel::{Receiver, Sender};

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PBT_APMRESUMESUSPEND,
        PBT_APMSUSPEND, PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
        WM_CLOSE, WM_DISPLAYCHANGE, WM_ENDSESSION, WM_POWERBROADCAST, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    },
};

/// Events that the platform event pump forwards to the daemon orchestrator.
#[derive(Debug, Clone)]
pub enum HostEvent {
    /// Explorer restarted; all host HWNDs are invalid. Full reattach required.
    ExplorerRestarted,

    /// Display configuration changed (monitor added/removed/moved/resized).
    DisplayChanged,

    /// Power / session state change.
    PerformanceHint(PerformanceProfile),

    /// Graceful shutdown requested (e.g. WM_ENDSESSION, SIGTERM-equivalent).
    ShutdownRequested,
}

/// Handle to signal the event pump thread to shut down cleanly.
pub struct PumpHandle {
    hwnd: Arc<Mutex<Option<isize>>>,
}

impl PumpHandle {
    /// Signal the event pump message loop to exit by posting WM_CLOSE
    /// to its hidden window. On non-Windows this is a no-op.
    pub fn shutdown(&self) {
        #[cfg(target_os = "windows")]
        if let Some(hwnd_ptr) = *self.hwnd.lock().unwrap() {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(hwnd_ptr as *mut std::ffi::c_void)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }
}

/// Drives the Win32 message loop on a dedicated thread.
///
/// Emits `HostEvent`s over a `crossbeam_channel` to the daemon orchestrator.
pub struct EventPump {
    sender: Sender<HostEvent>,
    pub receiver: Receiver<HostEvent>,
    hwnd: Arc<Mutex<Option<isize>>>,
}

impl EventPump {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let hwnd = Arc::new(Mutex::new(None));
        Self {
            sender,
            receiver,
            hwnd,
        }
    }

    /// Spawn the event pump thread running a Win32 message loop.
    /// Returns a `PumpHandle` for clean shutdown and the thread handle.
    pub fn spawn(self) -> (PumpHandle, std::thread::JoinHandle<()>) {
        let sender = self.sender.clone();
        let hwnd = self.hwnd.clone();
        let handle = std::thread::Builder::new()
            .name("aura-event-pump".into())
            .spawn(move || {
                #[cfg(target_os = "windows")]
                run_message_loop(sender, hwnd);

                #[cfg(not(target_os = "windows"))]
                {
                    let _ = sender;
                    let _ = hwnd;
                    tracing::warn!("EventPump not supported on non-Windows platform");
                }
            })
            .expect("failed to spawn event pump thread");
        (PumpHandle { hwnd: self.hwnd }, handle)
    }
}

impl Default for EventPump {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
thread_local! {
    static EVENT_SENDER: std::cell::Cell<Option<Sender<HostEvent>>> = const { std::cell::Cell::new(None) };
    static TASKBAR_CREATED_MSG: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[cfg(target_os = "windows")]
fn run_message_loop(sender: Sender<HostEvent>, hwnd_shared: Arc<Mutex<Option<isize>>>) {
    let taskbar_created = unsafe { RegisterWindowMessageW(windows::core::w!("TaskbarCreated")) };

    EVENT_SENDER.with(|s| s.set(Some(sender)));
    TASKBAR_CREATED_MSG.with(|m| m.set(taskbar_created));

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = windows::core::w!("AuraEventPumpClass");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            windows::core::w!("AuraEventPump"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(windows::Win32::Foundation::HINSTANCE(hinstance.0)),
            None,
        );

        if hwnd.is_err() {
            tracing::error!("Failed to create EventPump message window");
            return;
        }
        let hwnd = hwnd.unwrap();
        *hwnd_shared.lock().unwrap() = Some(hwnd.0 as isize);

        let mut power_mgr = crate::power::PowerManager::new();
        power_mgr.register(hwnd);

        tracing::info!(
            "EventPump message loop started (TaskbarCreated msg: {})",
            taskbar_created
        );

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            DispatchMessageW(&msg);
        }

        power_mgr.unregister(hwnd);
        tracing::info!("EventPump message loop terminated");
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_created = TASKBAR_CREATED_MSG.with(|m| m.get());

    if msg == taskbar_created && taskbar_created != 0 {
        tracing::info!("EventPump: TaskbarCreated broadcast received");
        EVENT_SENDER.with(|s| {
            if let Some(sender) = s.take() {
                let _ = sender.send(HostEvent::ExplorerRestarted);
                s.set(Some(sender));
            }
        });
        return LRESULT(0);
    }

    match msg {
        WM_DISPLAYCHANGE => {
            tracing::info!("EventPump: WM_DISPLAYCHANGE received");
            EVENT_SENDER.with(|s| {
                if let Some(sender) = s.take() {
                    let _ = sender.send(HostEvent::DisplayChanged);
                    s.set(Some(sender));
                }
            });
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            let event = wparam.0 as u32;
            if event == PBT_APMSUSPEND {
                tracing::info!("EventPump: System suspending");
                EVENT_SENDER.with(|s| {
                    if let Some(sender) = s.take() {
                        let profile = crate::power::PowerMonitor::profile_for_event(
                            crate::power::PowerEvent::DisplayOff,
                        );
                        let _ = sender.send(HostEvent::PerformanceHint(profile));
                        s.set(Some(sender));
                    }
                });
            } else if event == PBT_APMRESUMESUSPEND {
                tracing::info!("EventPump: System resumed");
                EVENT_SENDER.with(|s| {
                    if let Some(sender) = s.take() {
                        let profile = crate::power::PowerMonitor::profile_for_event(
                            crate::power::PowerEvent::DisplayOn,
                        );
                        let _ = sender.send(HostEvent::PerformanceHint(profile));
                        s.set(Some(sender));
                    }
                });
            } else if event == 0x8013 {
                // PBT_POWERSETTINGCHANGE
                if lparam.0 != 0 {
                    let setting = unsafe {
                        &*(lparam.0 as *const windows::Win32::System::Power::POWERBROADCAST_SETTING)
                    };
                    let is_on = setting.Data[0] != 0;
                    let p_event = if is_on {
                        crate::power::PowerEvent::DisplayOn
                    } else {
                        crate::power::PowerEvent::DisplayOff
                    };
                    tracing::info!("EventPump: Display state changed (is_on: {})", is_on);
                    EVENT_SENDER.with(|s| {
                        if let Some(sender) = s.take() {
                            let profile = crate::power::PowerMonitor::profile_for_event(p_event);
                            let _ = sender.send(HostEvent::PerformanceHint(profile));
                            s.set(Some(sender));
                        }
                    });
                }
            }
            LRESULT(1)
        }
        0x02B1 => {
            // WM_WTSSESSION_CHANGE
            let code = wparam.0 as u32;
            let p_event = match code {
                0x7 => Some(crate::power::PowerEvent::SessionLocked),   // WTS_SESSION_LOCK
                0x8 => Some(crate::power::PowerEvent::SessionUnlocked), // WTS_SESSION_UNLOCK
                _ => None,
            };
            if let Some(p_event) = p_event {
                tracing::info!("EventPump: Session state changed ({:?})", p_event);
                EVENT_SENDER.with(|s| {
                    if let Some(sender) = s.take() {
                        let profile = crate::power::PowerMonitor::profile_for_event(p_event);
                        let _ = sender.send(HostEvent::PerformanceHint(profile));
                        s.set(Some(sender));
                    }
                });
            }
            LRESULT(0)
        }
        WM_ENDSESSION | WM_CLOSE => {
            tracing::info!("EventPump: Shutdown requested");
            EVENT_SENDER.with(|s| {
                if let Some(sender) = s.take() {
                    let _ = sender.send(HostEvent::ShutdownRequested);
                    s.set(Some(sender));
                }
            });
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
