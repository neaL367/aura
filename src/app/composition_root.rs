use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use windows::Win32::Foundation::HWND;
use crate::domain::fit_mode::FitMode;
use crate::domain::monitor::MonitorId;
use crate::config::model::AppConfig;
use crate::monitor::manager::MonitorManager;
use crate::renderer::d3d11_device::D3d11Device;
use crate::renderer::texture::TextureRenderer;
use crate::platform::windows::workerw;
use crate::platform::windows::window_class::WallpaperWindow;
use crate::wallpaper::image_wallpaper::ImageWallpaper;
use crate::utils::error::Result;
use tracing::{info, error};

pub struct MonitorPipeline {
    pub wallpaper_window: WallpaperWindow,
    pub image_wallpaper: ImageWallpaper,
}

/// The composition root responsible for wiring and initializing components for Milestone 2.
pub struct CompositionRoot {
    pub device: Arc<D3d11Device>,
    pub renderer: Arc<TextureRenderer>,
    pub monitor_manager: MonitorManager,
    pub pipelines: HashMap<MonitorId, MonitorPipeline>,
    pub notification_window: crate::platform::windows::messages::NotificationWindow,
    pub parent_hwnd: HWND,
    pub is_standalone: bool,
}

impl CompositionRoot {
    /// Bootstraps the rendering pipeline, resolves the WorkerW parent, registers the child window
    /// for each active monitor, and performs initial frames rendering.
    pub fn new(config: &AppConfig) -> Result<Self> {
        // 1. Create shared D3D11 device & context
        info!("Initializing shared D3D11 device...");
        let device = Arc::new(D3d11Device::new()?);

        // 2. Initialize texture quad renderer (shader compilation)
        info!("Initializing texture quad renderer...");
        let renderer = Arc::new(TextureRenderer::new(&device.device)?);

        // 3. Find parent WorkerW window via the handshake
        info!("Resolving wallpaper parent window via WorkerW handshake...");
        let (parent_hwnd, is_standalone) = match workerw::get_wallpaper_parent() {
            Ok((hwnd, _)) => (hwnd, false),
            Err(e) => {
                tracing::warn!("WorkerW handshake failed: {:?}.", e);

                // Extract window count from error string to check for headless test environment
                let err_str = e.to_string();
                let is_headless = err_str.contains("windows=0");

                if !is_headless {
                    tracing::error!("WorkerW handshake failed on interactive desktop. Falling back to native system wallpaper.");
                    let default_path = PathBuf::from(r"C:\Windows\Web\Wallpaper\Windows\img0.jpg");
                    let wp_path = config.monitors.first()
                        .map(|m| m.wallpaper_path.clone())
                        .unwrap_or(default_path);
                    if let Err(err) = crate::platform::windows::window_class::set_desktop_wallpaper_native(&wp_path) {
                        tracing::error!("Failed to apply native desktop fallback wallpaper: {:?}", err);
                    }
                    return Err(crate::utils::error::AppError::Platform(
                        "WorkerW handshake failed on interactive desktop. Native fallback wallpaper applied.".to_string()
                    ));
                } else {
                    tracing::info!("Headless/Non-interactive session detected (0 windows). Falling back to standalone topmost window for testing.");
                    (windows::Win32::Foundation::HWND::default(), true)
                }
            }
        };

        // 4. Initialize Notification Window
        let notification_window = crate::platform::windows::messages::NotificationWindow::create()?;

        // 5. Initialize MonitorManager and enumerate active monitors
        let mut monitor_manager = MonitorManager::new();
        let active_ids = monitor_manager.initialize()?;

        let pipelines = HashMap::new();

        let mut root = Self {
            device,
            renderer,
            monitor_manager,
            pipelines,
            notification_window,
            parent_hwnd,
            is_standalone,
        };

        // 5. Create a pipeline for each active monitor
        for id in active_ids {
            if let Err(err) = root.add_monitor_pipeline(id, config) {
                error!("Failed to create pipeline for monitor: {:?}", err);
            }
        }

        Ok(root)
    }

    /// Creates a pipeline for a specific monitor (HWND creation, swapchain creation, initial render).
    pub fn add_monitor_pipeline(&mut self, id: MonitorId, config: &AppConfig) -> Result<()> {
        let info = match self.monitor_manager.current(&id) {
            Some(inf) => inf,
            None => return Err(crate::utils::error::AppError::Platform(format!("Monitor info not found for ID {:?}", id))),
        };

        let width = (info.rect.right - info.rect.left) as u32;
        let height = (info.rect.bottom - info.rect.top) as u32;

        let default_wallpaper = PathBuf::from(r"C:\Windows\Web\Wallpaper\Windows\img0.jpg");
        let (wp_path, fit_mode) = if let Some(m_cfg) = config.monitors.iter().find(|m| m.monitor_id == id.0) {
            (m_cfg.wallpaper_path.clone(), m_cfg.fit_mode)
        } else {
            (default_wallpaper, FitMode::Fill)
        };

        info!("Creating wallpaper pipeline for monitor {:?} (GDI: {}, Bounds: {}x{} at {},{})",
            id, info.gdi_device_name, width, height, info.rect.left, info.rect.top);

        let wallpaper_window = if self.is_standalone {
            WallpaperWindow::create_standalone_at(info.rect.left, info.rect.top, width as i32, height as i32)?
        } else {
            WallpaperWindow::create(
                self.parent_hwnd,
                info.rect.left,
                info.rect.top,
                width as i32,
                height as i32,
            )?
        };

        let image_wallpaper = ImageWallpaper::new(
            &self.device.device,
            wallpaper_window.hwnd,
            width,
            height,
            wp_path,
            fit_mode,
        )?;

        image_wallpaper.render(&self.device.device, &self.device.context, &self.renderer)?;

        self.pipelines.insert(id, MonitorPipeline {
            wallpaper_window,
            image_wallpaper,
        });

        Ok(())
    }

    /// Syncs display change events from MonitorManager (adding, removing, or changing monitors).
    pub fn sync_monitors(&mut self, config: &AppConfig) -> Result<()> {
        let events = self.monitor_manager.sync()?;
        info!("Display sync: sync produced {} events", events.len());
        for event in events {
            match event {
                crate::domain::events::AppEvent::MonitorAdded(id) => {
                    info!("Display sync: Monitor added: {:?}", id);
                    if let Err(err) = self.add_monitor_pipeline(id, config) {
                        error!("Failed to add pipeline during display sync: {:?}", err);
                    }
                }
                crate::domain::events::AppEvent::MonitorRemoved(id) => {
                    info!("Display sync: Monitor removed: {:?}", id);
                    self.pipelines.remove(&id);
                }
                crate::domain::events::AppEvent::MonitorChanged(id, geometry) => {
                    info!("Display sync: Monitor changed: {:?} (geometry: {:?})", id, geometry);
                    if let Some(pipeline) = self.pipelines.get_mut(&id) {
                        // Move and resize the Win32 child window
                        let x = geometry.x;
                        let y = geometry.y;
                        let w = geometry.width as i32;
                        let h = geometry.height as i32;
                        unsafe {
                            let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
                                pipeline.wallpaper_window.hwnd,
                                HWND::default(),
                                x,
                                y,
                                w,
                                h,
                                windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
                            );
                        }
                        // Resize swapchain buffers
                        if let Err(err) = pipeline.image_wallpaper.resize(&self.device.device, geometry.width as u32, geometry.height as u32) {
                            error!("Failed to resize swapchain buffers for monitor {:?}: {:?}", id, err);
                        }
                        // Trigger render
                        if let Err(err) = pipeline.image_wallpaper.render(&self.device.device, &self.device.context, &self.renderer) {
                            error!("Failed to render frame after monitor change: {:?}", err);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
