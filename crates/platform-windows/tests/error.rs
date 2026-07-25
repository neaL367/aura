#[cfg(target_os = "windows")]
mod windows_tests {
    use aura_platform_windows::PlatformError;

    #[test]
    fn display_workerw_not_found() {
        let err = PlatformError::WorkerWNotFound;
        assert!(err.to_string().contains("WorkerW not found"));
    }

    #[test]
    fn display_already_running() {
        let err = PlatformError::AlreadyRunning;
        assert!(err.to_string().contains("another instance"));
    }

    #[test]
    fn display_no_monitors() {
        let err = PlatformError::NoMonitors;
        assert!(err.to_string().contains("no monitors found"));
    }

    #[test]
    fn display_monitor_enum() {
        let err = PlatformError::MonitorEnum("test msg".into());
        let msg = err.to_string();
        assert!(msg.contains("monitor enumeration failed"));
        assert!(msg.contains("test msg"));
    }

    #[test]
    fn display_media_foundation() {
        let err = PlatformError::MediaFoundation(0x80004005);
        let msg = err.to_string();
        assert!(msg.contains("Media Foundation error"));
        assert!(msg.contains("0x80004005"));
    }

    #[test]
    fn display_window_creation() {
        let err = PlatformError::WindowCreation;
        assert!(err.to_string().contains("window creation failed"));
    }

    #[test]
    fn debug_all_variants() {
        let variants: Vec<PlatformError> = vec![
            PlatformError::WorkerWNotFound,
            PlatformError::AlreadyRunning,
            PlatformError::NoMonitors,
            PlatformError::MonitorEnum("dbg".into()),
            PlatformError::MediaFoundation(0),
            PlatformError::WindowCreation,
        ];
        for v in &variants {
            let _debug = format!("{:?}", v);
        }
    }
}
