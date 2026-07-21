use std::time::Instant;
use tracing::info;

/// PerfMonitor tracks rendering frame rates, latency, and performance hints.
pub(crate) struct PerfMonitor {
    frame_count: u64,
    last_log: Instant,
    fps: f32,
    frame_time_ms: f32,
}

impl PerfMonitor {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            last_log: Instant::now(),
            fps: 0.0,
            frame_time_ms: 0.0,
        }
    }

    pub fn record_frame(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_log.elapsed();
        if elapsed >= std::time::Duration::from_secs(5) {
            let secs = elapsed.as_secs_f32();
            self.fps = self.frame_count as f32 / secs;
            self.frame_time_ms = (secs / self.frame_count as f32) * 1000.0;
            info!(
                fps = format!("{:.1}", self.fps),
                frame_time_ms = format!("{:.2}", self.frame_time_ms),
                "Aura performance metrics"
            );
            self.frame_count = 0;
            self.last_log = Instant::now();
        }
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }

    pub fn frame_time_ms(&self) -> f32 {
        self.frame_time_ms
    }
}
