//! Device / sensor settings persisted as `config.json` beside the exe.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::flash_profile::FlashProfile;
use super::sensor_mapping::SensorMapping;
use crate::helpers::storage_helper;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub interval_ms: u64,
    pub flash: FlashProfile,
    pub sensors: SensorMapping,
    /// Optional override; when unset the daemon matches by product name / known VID:PID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_id: Option<u16>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            flash: FlashProfile::default(),
            sensors: SensorMapping::default(),
            vendor_id: None,
            product_id: None,
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        storage_helper::read_json(path)
    }

    pub fn load_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        storage_helper::write_json(path, self)
    }

    /// Load `config.json` (or path), then optionally merge official flashValue / config.
    pub fn resolve(config_path: Option<PathBuf>, official_dir: Option<PathBuf>) -> Result<Self> {
        let path = config_path.unwrap_or_else(|| storage_helper::config_path(&storage_helper::data_dir()));
        let mut cfg = Self::load_or_default(&path)?;

        if let Some(official) = official_dir {
            Self::merge_official(&mut cfg, &official)?;
        }
        Ok(cfg)
    }

    fn merge_official(cfg: &mut Self, official: &Path) -> Result<()> {
        let flash_path = official.join("flashValue");
        if flash_path.exists() {
            let line = fs::read_to_string(&flash_path)?;
            if let Ok(flash) = FlashProfile::parse_line(&line) {
                tracing::info!(
                    product = %flash.product,
                    index = flash.index,
                    flags = flash.flags,
                    "loaded flash profile from official/flashValue"
                );
                cfg.flash = flash;
            }
        }

        let mapping_path = official.join("config");
        if mapping_path.exists() {
            if let Ok(text) = fs::read_to_string(&mapping_path) {
                cfg.sensors.apply_official_config(&text);
            }
        }
        Ok(())
    }
}
