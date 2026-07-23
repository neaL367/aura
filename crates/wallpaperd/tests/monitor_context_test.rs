#[cfg(target_os = "windows")]
mod windows_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use aura_core::monitor::MonitorId;
    use aura_platform_windows::host_window::HostWindow;
    use wallpaperd::MonitorContext;

    fn make_context() -> MonitorContext {
        let host = HostWindow::create().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        MonitorContext::new(
            MonitorId::from_device_path(r"\\.\DISPLAY1"),
            host,
            handle,
            shutdown,
            Arc::new(AtomicBool::new(false)),
            1920, 1080, 0, 0,
        )
    }

    #[test]
    fn monitor_context_new_initial_state() {
        let ctx = make_context();
        assert!(!ctx.pause_flag.load(Ordering::Relaxed));
        assert_eq!(ctx.width, 1920);
        assert_eq!(ctx.height, 1080);
        assert_eq!(ctx.x, 0);
        assert_eq!(ctx.y, 0);
    }

    #[test]
    fn monitor_context_set_paused_updates_flag() {
        let ctx = make_context();
        ctx.set_paused(true);
        assert!(ctx.pause_flag.load(Ordering::Relaxed));
        ctx.set_paused(false);
        assert!(!ctx.pause_flag.load(Ordering::Relaxed));
    }
}
