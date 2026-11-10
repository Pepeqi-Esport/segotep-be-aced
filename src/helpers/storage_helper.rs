//! User data files living beside the exe (JSON + `.env`).
//!
//! Required on first run (same portable layout as s3-manager):
//! - `theme.json` — UI theme preference
//! - `config.json` — device / sensor settings
//! - `update.json` — dismissed GitHub release version
//! - `.env` — local flags such as `ENABLE_UPDATE_CHECK`
//!
//! Also migrates legacy `be-aced.toml` when present.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::{AppConfig, FlashProfile, SensorMapping, ThemeConfig};
use crate::helpers::payload_helper;
use crate::helpers::update_helper::UpdatePrefs;
use crate::utils::theme::AppTheme;

pub const THEME_FILE: &str = "theme.json";
pub const CONFIG_FILE: &str = "config.json";
pub const UPDATE_FILE: &str = "update.json";
pub const ENV_FILE: &str = ".env";
pub const LEGACY_TOML: &str = "be-aced.toml";

const DEFAULT_ENV: &str = "\
# BeAced local settings (beside the exe, or project root when using cargo run)
# Set to true to enable GitHub Releases update checks.
ENABLE_UPDATE_CHECK=false
";

pub fn theme_path(dir: &Path) -> PathBuf {
    dir.join(THEME_FILE)
}

pub fn config_path(dir: &Path) -> PathBuf {
    dir.join(CONFIG_FILE)
}

pub fn update_path(dir: &Path) -> PathBuf {
    dir.join(UPDATE_FILE)
}

pub fn env_path(dir: &Path) -> PathBuf {
    dir.join(ENV_FILE)
}

pub fn legacy_toml_path(dir: &Path) -> PathBuf {
    dir.join(LEGACY_TOML)
}

/// Resolve install dir (folder of this exe), falling back to cwd.
pub fn data_dir() -> PathBuf {
    payload_helper::install_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Create missing sidecar files beside the exe (`theme.json`, `config.json`,
/// `update.json`, `.env`). Safe to call on every launch.
pub fn ensure_sidecar_config_files() {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = migrate_legacy_toml(&dir);

    let theme_file = theme_path(&dir);
    if !theme_file.is_file() {
        let _ = ThemeConfig::default().save(&theme_file);
    }

    let config_file = config_path(&dir);
    if !config_file.is_file() {
        let _ = AppConfig::default().save(&config_file);
    }

    let update_file = update_path(&dir);
    if !update_file.is_file() {
        let _ = UpdatePrefs::default().save();
    }

    let env_file = env_path(&dir);
    if !env_file.is_file() {
        let _ = fs::write(&env_file, DEFAULT_ENV);
    }
}

/// Load `.env` from the current working directory and/or beside the exe.
/// Existing process env vars are never overwritten; earlier files win.
pub fn load_dotenv() {
    let _ = dotenvy::dotenv();
    let beside = env_path(&data_dir());
    if beside.is_file() {
        let _ = dotenvy::from_path(&beside);
    }
}

/// Write missing JSON defaults beside the exe and migrate legacy TOML once.
pub fn ensure_user_data() -> Result<(PathBuf, ThemeConfig, AppConfig)> {
    ensure_sidecar_config_files();

    let dir = data_dir();
    fs::create_dir_all(&dir).with_context(|| format!("create data dir {}", dir.display()))?;

    let theme_file = theme_path(&dir);
    let theme = if theme_file.is_file() {
        ThemeConfig::load(&theme_file)?
    } else {
        ThemeConfig::default()
    };

    let config = AppConfig::load_or_default(&config_path(&dir))?;

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
