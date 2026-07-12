use crate::utils::error::Result;
use crate::domain::monitor::{MonitorId, MonitorGeometry};
use crate::config::model::AppConfig;

/// Decoupled trait for loading and saving the application configuration.
pub trait ConfigStore: Send + Sync {
    /// Loads the configuration, returning default values if no configuration exists yet.
    fn load(&self) -> Result<AppConfig>;
    /// Saves the configuration atomically.
    fn save(&self, config: &AppConfig) -> Result<()>;
}

/// Decoupled trait for querying physical monitor information.
pub trait MonitorProvider: Send + Sync {
    /// Lists all currently active and connected monitors with their stable IDs and geometries.
    fn get_connected_monitors(&self) -> Result<Vec<(MonitorId, MonitorGeometry)>>;
}
