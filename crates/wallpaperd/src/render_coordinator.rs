use aura_platform_windows::host_window::HostWindow;
use aura_platform_windows::workerw::attach_to_workerw;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Per-monitor state owned by RenderCoordinator.
pub(crate) struct MonitorContext {
    pub host_window: HostWindow,
    pub render_thread: Option<std::thread::JoinHandle<()>>,
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
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.pause_flag.store(paused, Ordering::Relaxed);
    }
}

/// Manages all per-monitor render threads and windows.
pub(crate) struct RenderCoordinator {
    monitors: Vec<MonitorContext>,
}

impl RenderCoordinator {
    pub fn new(monitors: Vec<MonitorContext>) -> Self {
        Self { monitors }
    }

    pub fn monitor_count(&self) -> usize {
        self.monitors.len()
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

    pub fn shutdown(&mut self) {
        for ctx in &self.monitors {
            ctx.shutdown_flag.store(true, Ordering::Relaxed);
        }
        for ctx in &mut self.monitors {
            if let Some(handle) = ctx.render_thread.take() {
                let _ = handle.join();
            }
        }
    }
}
