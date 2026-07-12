use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use crate::config::model::AppConfig;
use crate::config::migrations;
use crate::domain::traits::ConfigStore;
use crate::utils::error::Result;

pub struct JsonConfigStore {
    path: PathBuf,
}

impl JsonConfigStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigStore for JsonConfigStore {
    fn load(&self) -> Result<AppConfig> {
        if !self.path.exists() {
            let default_config = AppConfig::default();
            self.save(&default_config)?;
            return Ok(default_config);
        }

        let file = File::open(&self.path)?;
        let raw_json: serde_json::Value = serde_json::from_reader(file)?;
        let migrated_json = migrations::migrate(raw_json);
        let config: AppConfig = serde_json::from_value(migrated_json)?;
        Ok(config)
    }

    fn save(&self, config: &AppConfig) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Create temporary file path next to the config file
        let mut temp_path = self.path.clone();
        temp_path.set_extension("json.tmp");

        {
            let mut temp_file = File::create(&temp_path)?;
            let json_str = serde_json::to_string_pretty(config)?;
            temp_file.write_all(json_str.as_bytes())?;
            temp_file.sync_all()?;
        }

        // Atomically rename over target file
        fs::rename(&temp_path, &self.path)?;
        Ok(())
    }
}
