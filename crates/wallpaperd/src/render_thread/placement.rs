use aura_core::monitor::MonitorInfo;
use aura_platform_windows::host_window::HostWindow;

use crate::daemon::DaemonError;

#[cfg(target_os = "windows")]
pub fn setup_host_window_placement(
    info: &MonitorInfo,
    workerw: windows::Win32::Foundation::HWND,
) -> Result<HostWindow, DaemonError> {
    let host_window = HostWindow::create()?;
    if !workerw.0.is_null() {
        if let Err(e) =
            aura_platform_windows::workerw::attach_to_workerw(host_window.hwnd(), workerw)
        {
            tracing::error!("Failed to attach window to WorkerW: {}", e);
        } else {
            unsafe {
                use windows::Win32::Foundation::{POINT, RECT};
                use windows::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient};
                use windows::Win32::UI::WindowsAndMessaging::{
                    GetWindowRect, IsWindowVisible, MoveWindow, SW_SHOW, ShowWindow,
                };
                let hwnd = host_window.hwnd();
                let mut pt = POINT {
                    x: info.x,
                    y: info.y,
                };

                // Explicitly check ScreenToClient return value (BOOL) to prevent corrupt coordinate placement
                if !ScreenToClient(workerw, &mut pt).as_bool() {
                    tracing::warn!(
                        "ScreenToClient failed for WorkerW {:?}, monitor {}; coordinates ({}, {}) remain unmodified",
                        workerw.0,
                        info.id,
                        info.x,
                        info.y
                    );
                }

                if let Err(e) = MoveWindow(
                    hwnd,
                    pt.x,
                    pt.y,
                    info.width as i32,
                    info.height as i32,
                    true,
                ) {
                    tracing::warn!(
                        "MoveWindow failed for monitor {} host window: {}",
                        info.id,
                        e
                    );
                }

                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = InvalidateRect(Some(hwnd), None, true);

                let visible = IsWindowVisible(hwnd).as_bool();
                let mut wrect = RECT::default();
                let rect_ok = GetWindowRect(hwnd, &mut wrect).is_ok();
                if rect_ok {
                    tracing::info!(
                        "Monitor {} host window placed at client-relative ({}, {}), size {}x{}; resulting screen rect ({},{})-({},{}) visible={}",
                        info.id,
                        pt.x,
                        pt.y,
                        info.width,
                        info.height,
                        wrect.left,
                        wrect.top,
                        wrect.right,
                        wrect.bottom,
                        visible
                    );
                } else {
                    tracing::warn!(
                        "Monitor {} host window placed at ({}, {}), size {}x{}, but GetWindowRect failed; visible={}",
                        info.id,
                        pt.x,
                        pt.y,
                        info.width,
                        info.height,
                        visible
                    );
                }
            }
        }
    } else {
        // No valid WorkerW/Progman target at all — fall back to an unparented
        // top-level window positioned behind Progman in the top-level z-order.
        if let Err(e) = aura_platform_windows::workerw::attach_topmost_bottom(
            host_window.hwnd(),
            info.x,
            info.y,
            info.width as i32,
            info.height as i32,
        ) {
            tracing::error!("Top-level fallback placement failed: {}", e);
        }
    }
    Ok(host_window)
}
