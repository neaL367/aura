use std::collections::HashMap;
use std::sync::Arc;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};
use crate::app::composition_root::MonitorPipeline;
use crate::config::model::AppConfig;
use crate::domain::fit_mode::FitMode;
use crate::domain::monitor::MonitorId;
use crate::domain::traits::ConfigStore;
use crate::utils::error::{Result, AppError};
use crate::renderer::texture::TextureRenderer;

/// Coordinates live assignment of wallpapers to physical display swapchains,
/// updating monitor renderer state and committing config adjustments to disk.
pub struct SetWallpaperService<'a> {
    pub device: ID3D11Device,
    pub device_context: ID3D11DeviceContext,
    pub renderer: Arc<TextureRenderer>,
    pub pipelines: &'a mut HashMap<MonitorId, MonitorPipeline>,
    pub config: &'a mut AppConfig,
    pub store: &'a dyn ConfigStore,
}

impl<'a> SetWallpaperService<'a> {
    /// Decodes the target image, replaces the texture on the target pipeline,
    /// redraws immediately, and updates the application configuration in the registry.
    pub fn assign(
        &mut self,
        monitor_id: &MonitorId,
        wallpaper_id: &str,
        fit_mode: FitMode,
    ) -> Result<()> {
        let entry = self.config.library.iter()
            .find(|e| e.id == wallpaper_id)
            .ok_or_else(|| AppError::Config(format!("unknown wallpaper id {wallpaper_id}")))?;

        // Decode + upload happens BEFORE touching the live pipeline, so a
        // bad/corrupt file fails loudly without ever tearing down the
        // wallpaper that's currently working.
        let new_texture = crate::media::image::wic_decoder::load_texture_from_file(&self.device, &entry.path)?;

        let pipeline = self.pipelines.get_mut(monitor_id)
            .ok_or_else(|| AppError::Platform(format!("no pipeline for monitor {:?}", monitor_id)))?;

        pipeline.image_wallpaper.replace_texture(new_texture, fit_mode);
        
        // Render updated texture immediately
        pipeline.image_wallpaper.render(&self.device, &self.device_context, &self.renderer)?;

        // Update the monitor's configuration
        if let Some(m_cfg) = self.config.monitors.iter_mut().find(|m| m.monitor_id == monitor_id.0) {
            m_cfg.wallpaper_id = Some(wallpaper_id.to_string());
            m_cfg.fit_mode = fit_mode;
        } else {
            self.config.monitors.push(crate::config::model::MonitorConfig {
                monitor_id: monitor_id.0.clone(),
                wallpaper_id: Some(wallpaper_id.to_string()),
                fit_mode,
                ..Default::default()
            });
        }

        self.store.save(self.config)?;
        Ok(())
    }

    /// Updates the fit mode on the live pipeline, redraws the display layout, and saves the settings.
    pub fn set_fit_mode(
        &mut self,
        monitor_id: &MonitorId,
        fit_mode: FitMode,
    ) -> Result<()> {
        let pipeline = self.pipelines.get_mut(monitor_id)
            .ok_or_else(|| AppError::Platform(format!("no pipeline for monitor {:?}", monitor_id)))?;

        pipeline.image_wallpaper.set_fit_mode(fit_mode);
        
        // Render updated fit mode layout immediately
        pipeline.image_wallpaper.render(&self.device, &self.device_context, &self.renderer)?;

        if let Some(m_cfg) = self.config.monitors.iter_mut().find(|m| m.monitor_id == monitor_id.0) {
            m_cfg.fit_mode = fit_mode;
        }

        self.store.save(self.config)?;
        Ok(())
    }
}
