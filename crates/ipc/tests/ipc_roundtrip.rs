use aura_core::monitor::MonitorId;
use aura_core::wallpaper::WallpaperId;
use aura_ipc::protocol::{DaemonStatus, IpcMessage, PROTOCOL_VERSION, Request, Response};

#[test]
fn test_request_serialization_roundtrip() {
    let requests = vec![
        Request::GetStatus,
        Request::ListWallpapers,
        Request::AssignWallpaper {
            monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
            wallpaper_id: WallpaperId::new(),
        },
        Request::RemoveAssignment {
            monitor_id: MonitorId::from_device_path(r"\\.\DISPLAY1"),
        },
        Request::PauseAll,
        Request::ResumeAll,
        Request::RefreshLibrary,
        Request::Shutdown,
    ];

    for req in requests {
        let msg = IpcMessage::new(req);
        let serialized = serde_json::to_string(&msg).expect("serialization failed");
        let deserialized: IpcMessage<Request> =
            serde_json::from_str(&serialized).expect("deserialization failed");

        assert_eq!(deserialized.version, PROTOCOL_VERSION);
    }
}

#[test]
fn test_response_serialization_roundtrip() {
    let status_resp = Response::Status(DaemonStatus {
        protocol_version: PROTOCOL_VERSION,
        active_monitors: 2,
        assigned_wallpapers: 1,
        is_paused: false,
    });

    let msg = IpcMessage::new(status_resp);
    let serialized = serde_json::to_string(&msg).expect("serialization failed");
    let deserialized: IpcMessage<Response> =
        serde_json::from_str(&serialized).expect("deserialization failed");

    assert_eq!(deserialized.version, PROTOCOL_VERSION);
}
