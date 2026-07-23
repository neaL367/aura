use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use aura_core::monitor::MonitorId;
use tracing::info;

struct MonitorPerfTracker {
    monitor_id: MonitorId,
    counter: Arc<AtomicU64>,
    last_count: u64,
    last_log: Instant,
}

/// PerfMonitor tracks per-monitor rendering frame rates and latency.
pub struct PerfMonitor {
    trackers: Vec<MonitorPerfTracker>,
}

impl PerfMonitor {
    pub fn new(counters: Vec<(MonitorId, Arc<AtomicU64>)>) -> Self {
        let now = Instant::now();
        let trackers = counters
            .into_iter()
            .map(|(id, counter)| MonitorPerfTracker {
                monitor_id: id,
                counter,
                last_count: 0,
                last_log: now,
            })
            .collect();
        Self { trackers }
    }

    pub fn log_if_interval(&mut self) {
        for tracker in &mut self.trackers {
            let elapsed = tracker.last_log.elapsed();
            if elapsed >= std::time::Duration::from_secs(5) {
                let current_count = tracker.counter.load(Ordering::Relaxed);
                let frames = current_count.saturating_sub(tracker.last_count);
                let secs = elapsed.as_secs_f32();
                let fps = if secs > 0.0 {
                    frames as f32 / secs
                } else {
                    0.0
                };
                let frame_time_ms = if frames > 0 {
                    (secs / frames as f32) * 1000.0
                } else {
                    0.0
                };

                info!(
                    monitor = %tracker.monitor_id,
                    fps = format!("{:.1}", fps),
                    frame_time_ms = format!("{:.2}", frame_time_ms),
                    "Aura performance metrics"
                );

                tracker.last_count = current_count;
                tracker.last_log = Instant::now();
            }
        }
    }
}
