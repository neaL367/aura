use aura_core::playback::PerformanceProfile;
use crossbeam_channel::{Receiver, Sender};

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PBT_APMRESUMESUSPEND,
        PBT_APMSUSPEND, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, WM_CLOSE,
        WM_DISPLAYCHANGE, WM_ENDSESSION, WM_POWERBROADCAST, WNDCLASSW, WS_OVERLAPPEDWINDOW,
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

/// Drives the Win32 message loop on a dedicated thread.
///
/// Emits `HostEvent`s over a `crossbeam_channel` to the daemon orchestrator.
pub struct EventPump {
    sender: Sender<HostEvent>,
    pub receiver: Receiver<HostEvent>,
}

impl EventPump {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }

    /// Spawn the event pump thread running a Win32 message loop.
    pub fn spawn(self) -> std::thread::JoinHandle<()> {
        let sender = self.sender.clone();
        std::thread::Builder::new()
            .name("aura-event-pump".into())
            .spawn(move || {
                #[cfg(target_os = "windows")]
                run_message_loop(sender);

                #[cfg(not(target_os = "windows"))]
                {
                    let _ = sender;
                    tracing::warn!("EventPump not supported on non-Windows platform");
                }
            })
            .expect("failed to spawn event pump thread")
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
fn run_message_loop(sender: Sender<HostEvent>) {
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

        tracing::info!(
            "EventPump message loop started (TaskbarCreated msg: {})",
            taskbar_created
        );

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            DispatchMessageW(&msg);
        }

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
                        let _ = sender.send(HostEvent::PerformanceHint(PerformanceProfile::Paused));
                        s.set(Some(sender));
                    }
                });
            } else if event == PBT_APMRESUMESUSPEND {
                tracing::info!("EventPump: System resumed");
                EVENT_SENDER.with(|s| {
                    if let Some(sender) = s.take() {
                        let _ =
                            sender.send(HostEvent::PerformanceHint(PerformanceProfile::Balanced));
                        s.set(Some(sender));
                    }
                });
            }
            LRESULT(1)
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
