use aura_core::monitor::MonitorId;
use aura_core::wallpaper::WallpaperId;
use wallpaperd::AssignmentManager;

fn mon(id: &str) -> MonitorId {
    MonitorId::from_device_path(id)
}

fn wal() -> WallpaperId {
    WallpaperId::new()
}

#[test]
fn new_is_empty() {
    let mgr = AssignmentManager::new();
    assert!(mgr.all().is_empty());
}

#[test]
fn assign_and_all() {
    let mut mgr = AssignmentManager::new();
    let m = mon(r"\\.\DISPLAY1");
    let w = wal();
    mgr.assign(m, w);
    assert_eq!(mgr.all().len(), 1);
    assert_eq!(mgr.get(&m), Some(&w));
}

#[test]
fn assign_overwrites() {
    let mut mgr = AssignmentManager::new();
    let m = mon(r"\\.\DISPLAY1");
    let w1 = wal();
    let w2 = wal();
    mgr.assign(m, w1);
    mgr.assign(m, w2);
    assert_eq!(mgr.get(&m), Some(&w2));
    assert_eq!(mgr.all().len(), 1);
}

#[test]
fn assign_multiple_monitors() {
    let mut mgr = AssignmentManager::new();
    let m1 = mon(r"\\.\DISPLAY1");
    let m2 = mon(r"\\.\DISPLAY2");
    let w1 = wal();
    let w2 = wal();
    mgr.assign(m1, w1);
    mgr.assign(m2, w2);
    assert_eq!(mgr.all().len(), 2);
}

#[test]
fn remove_existing() {
    let mut mgr = AssignmentManager::new();
    let m = mon(r"\\.\DISPLAY1");
    let w = wal();
    mgr.assign(m, w);
    let removed = mgr.remove(&m);
    assert_eq!(removed, Some(w));
    assert!(mgr.all().is_empty());
}

#[test]
fn remove_missing() {
    let mut mgr = AssignmentManager::new();
    let m = mon(r"\\.\DISPLAY1");
    assert!(mgr.remove(&m).is_none());
}

#[test]
fn get_existing() {
    let mut mgr = AssignmentManager::new();
    let m = mon(r"\\.\DISPLAY1");
    let w = wal();
    mgr.assign(m, w);
    assert_eq!(mgr.get(&m), Some(&w));
}

#[test]
fn get_missing() {
    let mgr = AssignmentManager::new();
    let m = mon(r"\\.\DISPLAY1");
    assert_eq!(mgr.get(&m), None);
}

#[test]
fn all_returns_current_snapshot() {
    let mut mgr = AssignmentManager::new();
    let m = mon(r"\\.\DISPLAY1");
    let w = wal();
    mgr.assign(m, w);
    let snapshot = mgr.all();
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot.contains_key(&m));
}
