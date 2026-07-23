use aura_core::monitor::MonitorId;
use aura_core::playback::PlaybackCommand;
use aura_core::wallpaper::{FitMode, MediaKind, WallpaperId};
use aura_ipc::protocol::{
    DaemonStatus, IpcMessage, PROTOCOL_VERSION, Request, Response, WallpaperEntry,
};

#[test]
fn test_request_serialization_roundtrip() {
    let requests = vec![
        Request::GetStatus,
        Request::ListWallpapers,
        Request::AssignWallpaper {
            monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
            wallpaper_id: WallpaperId::new(),
            fit_mode: Some(FitMode::Fill),
        },
        Request::SetFitMode {
            monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
            fit_mode: FitMode::Fit,
        },
        Request::RemoveAssignment {
            monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
        },
        Request::PauseAll,
        Request::ResumeAll,
        Request::RefreshLibrary,
        Request::AddScanPath {
            path: std::path::PathBuf::from(r"C:\Wallpapers"),
        },
        Request::RemoveScanPath {
            path: std::path::PathBuf::from(r"C:\Wallpapers"),
        },
        Request::GetConfig,
        Request::UpdateConfig {
            config: aura_core::config::AppConfig::default(),
        },
        Request::Shutdown,
        Request::SetPlayback {
            monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
            command: PlaybackCommand::Pause,
        },
    ];

    for req in requests {
        let msg = IpcMessage::new(req);
        let serialized = serde_json::to_string(&msg).expect("serialization failed");
        let deserialized: IpcMessage<Request> =
            serde_json::from_str(&serialized).expect("deserialization failed");

        assert_eq!(deserialized.version, PROTOCOL_VERSION);
        assert_eq!(deserialized.payload, msg.payload);
    }
}

#[test]
fn test_response_serialization_roundtrip() {
    let responses = vec![
        Response::Ok,
        Response::Error {
            reason: "something went wrong".to_string(),
        },
        Response::Status(DaemonStatus {
            protocol_version: PROTOCOL_VERSION,
            active_monitors: 2,
            assigned_wallpapers: 1,
            is_paused: false,
            monitors: vec![],
        }),
        Response::WallpaperList(vec![
            WallpaperEntry {
                id: WallpaperId::new(),
                path: std::path::PathBuf::from(r"C:\Wallpapers\test.png"),
                kind: MediaKind::Image,
                thumbnail_path: Some(std::path::PathBuf::from(r"C:\Thumbs\thumb1.jpg")),
            },
            WallpaperEntry {
                id: WallpaperId::new(),
                path: std::path::PathBuf::from(r"C:\Wallpapers\anim.gif"),
                kind: MediaKind::Gif,
                thumbnail_path: None,
            },
        ]),
        Response::Config(aura_core::config::AppConfig::default()),
    ];

    for resp in responses {
        let msg = IpcMessage::new(resp);
        let serialized = serde_json::to_string(&msg).expect("serialization failed");
        let deserialized: IpcMessage<Response> =
            serde_json::from_str(&serialized).expect("deserialization failed");

        assert_eq!(deserialized.version, PROTOCOL_VERSION);
        assert_eq!(deserialized.payload, msg.payload);
    }
}
