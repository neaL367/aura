#[cfg(target_os = "windows")]
mod windows_tests {
    use std::time::Duration;

    use aura_ipc::client::IpcClient;
    use aura_ipc::protocol::{DaemonStatus, MonitorSummary, PROTOCOL_VERSION, Request, Response};
    use aura_ipc::server::IpcServer;

    fn test_pipe_name() -> String {
        format!(
            r"\\.\pipe\aura-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[tokio::test]
    async fn test_ipc_getstatus_roundtrip() {
        let pipe_name = test_pipe_name();
        let handler = Box::new(|req: Request| -> Response {
            match req {
                Request::GetStatus => Response::Status(DaemonStatus {
                    protocol_version: PROTOCOL_VERSION,
                    active_monitors: 2,
                    assigned_wallpapers: 1,
                    is_paused: false,
                    monitors: vec![MonitorSummary {
                        id: aura_core::monitor::MonitorId::from_device_path(r"\\.\DISPLAY1"),
                        name: "Display 1".into(),
                    }],
                }),
                _ => Response::Error {
                    reason: "unexpected request".into(),
                },
            }
        });

        let server = IpcServer::on_pipe(handler, pipe_name.clone());
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        tokio::spawn(async move {
            let _ = server.serve(shutdown_rx).await;
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut client = IpcClient::connect_to(&pipe_name).await.unwrap();
        let response = client.send(Request::GetStatus).await.unwrap();

        match response {
            Response::Status(status) => {
                assert_eq!(status.protocol_version, PROTOCOL_VERSION);
                assert_eq!(status.active_monitors, 2);
                assert_eq!(status.assigned_wallpapers, 1);
                assert!(!status.is_paused);
                assert_eq!(status.monitors.len(), 1);
            }
            other => panic!("expected Status response, got {:?}", other),
        }

        shutdown_tx.send(true).unwrap();
    }

    #[tokio::test]
    async fn test_ipc_listwallpapers_roundtrip() {
        let pipe_name = test_pipe_name();
        let handler = Box::new(|req: Request| -> Response {
            match req {
                Request::ListWallpapers => Response::WallpaperList(vec![]),
                _ => Response::Error {
                    reason: "unexpected request".into(),
                },
            }
        });

        let server = IpcServer::on_pipe(handler, pipe_name.clone());
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        tokio::spawn(async move {
            let _ = server.serve(shutdown_rx).await;
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut client = IpcClient::connect_to(&pipe_name).await.unwrap();
        let response = client.send(Request::ListWallpapers).await.unwrap();

        match response {
            Response::WallpaperList(list) => assert!(list.is_empty()),
            other => panic!("expected WallpaperList, got {:?}", other),
        }

        shutdown_tx.send(true).unwrap();
    }
}
