use std::time::Duration;

use aura_platform_windows::host_window::HostWindow;
use aura_platform_windows::workerw::attach_to_workerw;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Per-monitor state owned by RenderCoordinator.
pub struct MonitorContext {
    pub monitor_id: aura_core::monitor::MonitorId,
    pub render_thread: Option<std::thread::JoinHandle<()>>,
    pub host_window: HostWindow,
    pub shutdown_flag: Arc<AtomicBool>,
    pub pause_flag: Arc<AtomicBool>,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

impl MonitorContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        monitor_id: aura_core::monitor::MonitorId,
        host_window: HostWindow,
        render_thread: std::thread::JoinHandle<()>,
        shutdown_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
    ) -> Self {
        Self {
            monitor_id,
            host_window,
            render_thread: Some(render_thread),
            shutdown_flag,
            pause_flag,
            width,
            height,
            x,
            y,
        }
    }

    pub fn attach_to_workerw(&self, workerw: windows::Win32::Foundation::HWND) {
        if let Err(e) = attach_to_workerw(self.host_window.hwnd(), workerw) {
            tracing::error!("Failed to attach window to WorkerW: {}", e);
            return;
        }
        unsafe {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
            use windows::Win32::UI::WindowsAndMessaging::{MoveWindow, SW_SHOW, ShowWindow};
            let hwnd = self.host_window.hwnd();
            let mut pt = POINT {
                x: self.x,
                y: self.y,
            };
            let _ = ScreenToClient(workerw, &mut pt);
            let _ = MoveWindow(
                hwnd,
                pt.x,
                pt.y,
                self.width as i32,
                self.height as i32,
                true,
            );
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = InvalidateRect(Some(hwnd), None, true);

            let visible = windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool();
            tracing::info!(
                "Monitor host window placed at ({}, {}) {}x{}, visible={}",
                pt.x,
                pt.y,
                self.width,
                self.height,
                visible
            );
        }
    }

    pub fn update_geometry(
        &mut self,
        workerw: windows::Win32::Foundation::HWND,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
        self.attach_to_workerw(workerw);
    }

    pub fn set_paused(&self, paused: bool) {
        self.pause_flag.store(paused, Ordering::Relaxed);
    }
}

/// Manages all per-monitor render threads and windows.
pub struct RenderCoordinator {
    monitors: Vec<MonitorContext>,
}

impl RenderCoordinator {
    pub fn new(monitors: Vec<MonitorContext>) -> Self {
        Self { monitors }
    }

    pub fn monitor_count(&self) -> usize {
        self.monitors.len()
    }

    pub fn active_monitor_ids(&self) -> Vec<aura_core::monitor::MonitorId> {
        self.monitors.iter().map(|m| m.monitor_id).collect()
    }

    /// Calculate the total bounding box `(min_x, min_y, total_w, total_h)` across all active monitors.
    pub fn virtual_desktop_bounds(&self) -> (i32, i32, u32, u32) {
        if self.monitors.is_empty() {
            return (0, 0, 1920, 1080);
        }
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for m in &self.monitors {
            min_x = min_x.min(m.x);
            min_y = min_y.min(m.y);
            max_x = max_x.max(m.x + m.width as i32);
            max_y = max_y.max(m.y + m.height as i32);
        }

        let total_w = (max_x - min_x).max(1) as u32;
        let total_h = (max_y - min_y).max(1) as u32;
        (min_x, min_y, total_w, total_h)
    }

    pub fn add_monitor(&mut self, context: MonitorContext) {
        self.monitors.push(context);
    }

    pub fn remove_monitor(&mut self, monitor_id: aura_core::monitor::MonitorId) {
        if let Some(pos) = self
            .monitors
            .iter()
            .position(|m| m.monitor_id == monitor_id)
        {
            let mut ctx = self.monitors.remove(pos);
            ctx.shutdown_flag.store(true, Ordering::Relaxed);
            if let Some(handle) = ctx.render_thread.take() {
                let _ = handle.join();
            }
        }
    }

    pub fn find_monitor_mut(
        &mut self,
        monitor_id: aura_core::monitor::MonitorId,
    ) -> Option<&mut MonitorContext> {
        self.monitors
            .iter_mut()
            .find(|m| m.monitor_id == monitor_id)
    }

    pub fn attach_all(&mut self, workerw: windows::Win32::Foundation::HWND) {
        for ctx in &self.monitors {
            ctx.attach_to_workerw(workerw);
        }
    }

    pub fn set_paused(&self, paused: bool) {
        for ctx in &self.monitors {
            ctx.set_paused(paused);
        }
    }

    /// Signal all render threads and wait for them with a timeout.
    /// Threads that don't finish within the timeout are detached.
    pub fn shutdown_with_timeout(&mut self, timeout: Duration) {
        let deadline = std::time::Instant::now() + timeout;
        for ctx in &self.monitors {
            ctx.shutdown_flag.store(true, Ordering::Relaxed);
        }
        for ctx in &mut self.monitors {
            if let Some(handle) = ctx.render_thread.take() {
                while std::time::Instant::now() < deadline {
                    if handle.is_finished() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                if handle.is_finished() {
                    let _ = handle.join();
                } else {
                    tracing::warn!(
                        "Shutdown timeout exceeded for {:?}, detaching render thread",
                        ctx.monitor_id
                    );
                }
            }
        }
    }
}
