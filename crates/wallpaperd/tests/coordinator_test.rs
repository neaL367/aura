#[cfg(target_os = "windows")]
mod windows_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use aura_core::monitor::MonitorId;
    use aura_platform_windows::host_window::HostWindow;
    use wallpaperd::{MonitorContext, RenderCoordinator};

    fn test_monitor_id(label: u32) -> MonitorId {
        MonitorId::from_device_path(&format!(r"\\.\DISPLAY{label}"))
    }

    fn make_context(id: MonitorId) -> MonitorContext {
        let host = HostWindow::create().unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let flag = shutdown.clone();
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        MonitorContext::new(
            id,
            host,
            handle,
            shutdown,
            Arc::new(AtomicBool::new(false)),
            1920, 1080, 0, 0,
        )
    }

    fn shutdown_all(mut coord: RenderCoordinator) {
        let ids: Vec<_> = coord.active_monitor_ids();
        for id in ids {
            coord.remove_monitor(id);
        }
    }

    #[test]
    fn coordinator_new_is_empty() {
        let coord = RenderCoordinator::new(vec![]);
        assert_eq!(coord.monitor_count(), 0);
        assert!(coord.active_monitor_ids().is_empty());
    }

    #[test]
    fn coordinator_add_increases_count() {
        let mut coord = RenderCoordinator::new(vec![]);
        coord.add_monitor(make_context(test_monitor_id(1)));
        assert_eq!(coord.monitor_count(), 1);
        shutdown_all(coord);
    }

    #[test]
    fn coordinator_add_two_monitors() {
        let mut coord = RenderCoordinator::new(vec![]);
        coord.add_monitor(make_context(test_monitor_id(1)));
        coord.add_monitor(make_context(test_monitor_id(2)));
        assert_eq!(coord.monitor_count(), 2);
        assert_eq!(coord.active_monitor_ids().len(), 2);
        shutdown_all(coord);
    }

    #[test]
    fn coordinator_remove_monitor_decreases_count() {
        let mut coord = RenderCoordinator::new(vec![]);
        let id = test_monitor_id(1);
        coord.add_monitor(make_context(id));
        assert_eq!(coord.monitor_count(), 1);
        coord.remove_monitor(id);
        assert_eq!(coord.monitor_count(), 0);
    }

    #[test]
    fn coordinator_active_monitor_ids_matches() {
        let mut coord = RenderCoordinator::new(vec![]);
        let id1 = test_monitor_id(1);
        let id2 = test_monitor_id(2);
        coord.add_monitor(make_context(id1));
        coord.add_monitor(make_context(id2));
        let ids = coord.active_monitor_ids();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        assert_eq!(ids.len(), 2);
        shutdown_all(coord);
    }

    #[test]
    fn coordinator_find_monitor_mut_existing() {
        let mut coord = RenderCoordinator::new(vec![]);
        let id = test_monitor_id(1);
        coord.add_monitor(make_context(id));
        let found = coord.find_monitor_mut(id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().monitor_id, id);
        shutdown_all(coord);
    }

    #[test]
    fn coordinator_find_monitor_mut_missing() {
        let mut coord = RenderCoordinator::new(vec![]);
        let id = test_monitor_id(1);
        assert!(coord.find_monitor_mut(id).is_none());
    }

    #[test]
    fn coordinator_set_paused_updates_all() {
        let mut coord = RenderCoordinator::new(vec![]);
        coord.add_monitor(make_context(test_monitor_id(1)));
        coord.add_monitor(make_context(test_monitor_id(2)));

        coord.set_paused(true);
        for id in coord.active_monitor_ids() {
            let ctx = coord.find_monitor_mut(id).unwrap();
            assert!(ctx.pause_flag.load(Ordering::Relaxed));
        }

        coord.set_paused(false);
        for id in coord.active_monitor_ids() {
            let ctx = coord.find_monitor_mut(id).unwrap();
            assert!(!ctx.pause_flag.load(Ordering::Relaxed));
        }

        shutdown_all(coord);
    }
}
