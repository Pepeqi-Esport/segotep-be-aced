//! User data files living beside the exe (JSON).
//!
//! Required on first run:
//! - `theme.json` — UI theme preference
//! - `config.json` — device / sensor settings
//!
//! Optional:
//! - `update.json` — dismissed GitHub release version
//!
//! Also migrates legacy `be-aced.toml` when present.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::{AppConfig, FlashProfile, SensorMapping, ThemeConfig};
use crate::helpers::payload_helper;
use crate::utils::theme::AppTheme;

pub const THEME_FILE: &str = "theme.json";
pub const CONFIG_FILE: &str = "config.json";
pub const UPDATE_FILE: &str = "update.json";
pub const LEGACY_TOML: &str = "be-aced.toml";

pub fn theme_path(dir: &Path) -> PathBuf {
    dir.join(THEME_FILE)
}

pub fn config_path(dir: &Path) -> PathBuf {
    dir.join(CONFIG_FILE)
}

pub fn update_path(dir: &Path) -> PathBuf {
    dir.join(UPDATE_FILE)
}

pub fn legacy_toml_path(dir: &Path) -> PathBuf {
    dir.join(LEGACY_TOML)
}

/// Resolve install dir (folder of this exe), falling back to cwd.
pub fn data_dir() -> PathBuf {
    payload_helper::install_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Write missing JSON defaults beside the exe and migrate legacy TOML once.
pub fn ensure_user_data() -> Result<(PathBuf, ThemeConfig, AppConfig)> {
    let dir = data_dir();
    fs::create_dir_all(&dir).with_context(|| format!("create data dir {}", dir.display()))?;

    migrate_legacy_toml(&dir)?;

    let theme_file = theme_path(&dir);
    let config_file = config_path(&dir);

    let theme = if theme_file.is_file() {
        ThemeConfig::load(&theme_file)?
    } else {
        let theme = ThemeConfig::default();
        theme.save(&theme_file)?;
        tracing::info!(path = %theme_file.display(), "created default theme.json");
        theme
    };

    let config = if config_file.is_file() {
        AppConfig::load(&config_file)?
    } else {
        let config = AppConfig::default();
        config.save(&config_file)?;
        tracing::info!(path = %config_file.display(), "created default config.json");
        config
    };

    Ok((dir, theme, config))
}

fn migrate_legacy_toml(dir: &Path) -> Result<()> {
    let toml_path = legacy_toml_path(dir);
    if !toml_path.is_file() {
        return Ok(());
    }

    let theme_file = theme_path(dir);
    let config_file = config_path(dir);
    if theme_file.is_file() && config_file.is_file() {
        return Ok(());
    }

    let text = fs::read_to_string(&toml_path)
        .with_context(|| format!("read legacy {}", toml_path.display()))?;
    let legacy: LegacyToml = toml::from_str(&text)
        .with_context(|| format!("parse legacy {}", toml_path.display()))?;

    if !theme_file.is_file() {
        let theme = ThemeConfig {
            theme: legacy.theme.unwrap_or_default(),
        };
        theme.save(&theme_file)?;
        tracing::info!(path = %theme_file.display(), "migrated theme from be-aced.toml");
    }

    if !config_file.is_file() {
        let config = AppConfig {
            interval_ms: legacy.interval_ms.unwrap_or(1000),
            flash: legacy.flash.unwrap_or_default(),
            sensors: legacy.sensors.unwrap_or_default(),
            vendor_id: legacy.vendor_id,
            product_id: legacy.product_id,
        };
        config.save(&config_file)?;
        tracing::info!(path = %config_file.display(), "migrated config from be-aced.toml");
    }

    Ok(())
}

/// Pretty-print JSON with trailing newline.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{text}\n")).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

#[derive(Debug, Deserialize)]
struct LegacyToml {
    interval_ms: Option<u64>,
    theme: Option<AppTheme>,
    flash: Option<FlashProfile>,
    sensors: Option<SensorMapping>,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
}
