use std::path::PathBuf;

use aura_core::config::{AppConfig, CONFIG_VERSION};
use tracing::{info, warn};

use crate::{error::StorageError, migration};

/// Reads and writes `AppConfig` to a TOML file.
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Resolve the default system configuration file path (`%APPDATA%\Aura\aura.toml`).
    pub fn default_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\AuraData"));
        base.join("Aura").join("aura.toml")
    }

    /// Load configuration from disk.
    ///
    /// - Missing file → returns `AppConfig::default()`.
    /// - Corrupt file → returns `StorageError::Corrupt`.
    /// - Old version  → migrates in memory and saves back.
    pub fn load(&self) -> Result<AppConfig, StorageError> {
        if !self.path.exists() {
            info!("No config file at {:?}; creating default config", self.path);
            let default_cfg = AppConfig::default();
            let _ = self.save(&default_cfg);
            return Ok(default_cfg);
        }

        let raw = std::fs::read_to_string(&self.path)?;
        let mut cfg: AppConfig = toml::from_str(&raw)
            .map_err(|e| StorageError::Corrupt(format!("TOML parse error: {}", e)))?;

        // Run migration if schema version is older.
        if cfg.version < CONFIG_VERSION {
            warn!(from = cfg.version, to = CONFIG_VERSION, "Migrating config");
            cfg = migration::migrate(cfg)?;
            self.save(&cfg)?;
        }

        Ok(cfg)
    }

    /// Write configuration to disk atomically (write to `.tmp`, then rename).
    pub fn save(&self, config: &AppConfig) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let serialised = toml::to_string_pretty(config)?;
        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, serialised)?;
        let _ = std::fs::remove_file(&self.path);
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}
