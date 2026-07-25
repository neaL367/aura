use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use aura_core::monitor::MonitorId;
use wallpaperd::PerfMonitor;

fn mon(id: &str) -> MonitorId {
    MonitorId::from_device_path(id)
}

#[test]
fn new_creates_trackers() {
    let counters = vec![
        (mon(r"\\.\DISPLAY1"), Arc::new(AtomicU64::new(0))),
        (mon(r"\\.\DISPLAY2"), Arc::new(AtomicU64::new(0))),
        (mon(r"\\.\DISPLAY3"), Arc::new(AtomicU64::new(0))),
    ];
    let pm = PerfMonitor::new(counters);
    // No public accessor for internal tracker count, so we just verify it
    // doesn't panic and can be called immediately.
    // (This is a smoke test — the struct is constructed successfully.)
    let _ = pm;
}

#[test]
fn log_if_interval_before_5s_no_panic() {
    let counter = Arc::new(AtomicU64::new(0));
    let counters = vec![(mon(r"\\.\DISPLAY1"), counter)];
    let mut pm = PerfMonitor::new(counters);
    // Calling immediately (<5s since creation) should be a no-op.
    pm.log_if_interval();
}

#[test]
fn log_if_interval_after_5s_no_panic() {
    let counter = Arc::new(AtomicU64::new(100));
    let counters = vec![(mon(r"\\.\DISPLAY1"), counter)];
    let mut pm = PerfMonitor::new(counters);
    // There's no way to fake Instant::now(), so we sleep.
    // We accept the 5s delay here because this is a correctness test.
    std::thread::sleep(Duration::from_secs(5));
    pm.log_if_interval();
}

#[test]
fn log_if_interval_zero_frames_no_panic() {
    let counter = Arc::new(AtomicU64::new(0));
    let counters = vec![(mon(r"\\.\DISPLAY1"), counter)];
    let mut pm = PerfMonitor::new(counters);
    std::thread::sleep(Duration::from_secs(5));
    pm.log_if_interval();
}
