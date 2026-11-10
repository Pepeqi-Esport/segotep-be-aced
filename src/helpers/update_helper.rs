//! Check GitHub Releases and apply in-place updates (`self_update`).

use std::process::Command;
use std::thread;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::helpers::storage_helper;
use crate::services::background_service;
use crate::services::sensor_service;

pub const REPO_OWNER: &str = "codenameryuu";
pub const REPO_NAME: &str = "segotep-be-aced";
pub const BIN_NAME: &str = "segotep-be-aced.exe";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UpdatePrefs {
    /// Remote version the user chose "Later" for (no leading `v`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed_version: Option<String>,
}

impl UpdatePrefs {
    pub fn load() -> Self {
        let path = storage_helper::update_path(&storage_helper::data_dir());
        if path.is_file() {
            storage_helper::read_json(&path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = storage_helper::update_path(&storage_helper::data_dir());
        storage_helper::write_json(&path, self)
    }

    pub fn dismiss(&mut self, version: &str) -> Result<()> {
        self.dismissed_version = Some(normalize_version(version));
        self.save()
    }

    pub fn clear_dismissed(&mut self) -> Result<()> {
        self.dismissed_version = None;
        self.save()
    }
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current: String,
    pub remote: String,
    pub newer: bool,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn normalize_version(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn configure_updater() -> Result<self_update::backends::github::Update> {
    self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .current_version(current_version())
        .no_confirm(true)
        .show_download_progress(false)
        .show_output(false)
        .build()
        .map_err(|e| anyhow!("configure updater: {e}"))
}

/// Fetch latest GitHub release and compare to the running binary.
pub fn check_latest() -> Result<UpdateInfo> {
    let updater = configure_updater()?;
    let current = current_version().to_string();
    match updater
        .is_update_available()
        .map_err(|e| anyhow!("GitHub release check failed: {e}"))?
    {
        Some(release) => Ok(UpdateInfo {
            current,
            remote: normalize_version(release.version()),
            newer: true,
        }),
        None => {
            // Still surface the latest tag when up to date (for status text).
            let remote = updater
                .get_latest_release()
                .ok()
                .and_then(|r| r.latest().map(|rel| normalize_version(rel.version())))
                .unwrap_or_else(|| current.clone());
            Ok(UpdateInfo {
                current,
                remote,
                newer: false,
            })
        }
    }
}

/// Startup auto-prompt: newer than us and not the dismissed remote version.
pub fn should_prompt_on_startup(info: &UpdateInfo, prefs: &UpdatePrefs) -> bool {
    if !info.newer {
        return false;
    }
    match &prefs.dismissed_version {
        Some(dismissed) => normalize_version(dismissed) != info.remote,
        None => true,
    }
}

fn prepare_for_replace() {
    background_service::stop();
    sensor_service::kill_all_sensor_bridges();
    // Give Windows a moment to release file locks (HID / bridge / daemon).
    thread::sleep(std::time::Duration::from_millis(300));
}

/// Download the latest release, replace this exe, then relaunch and exit.
pub fn apply_update() -> Result<()> {
    prepare_for_replace();

    let updater = configure_updater()?;
    let status = updater
        .update()
        .map_err(|e| anyhow!("update failed: {e}"))?;

    if status.is_updated() {
        tracing::info!(version = %status.version(), "binary updated");
        let mut prefs = UpdatePrefs::load();
        let _ = prefs.clear_dismissed();
        relaunch_and_exit().context("relaunch after update")?;
        Ok(())
    } else {
        Err(anyhow!("already up to date (v{})", status.version()))
    }
}

fn relaunch_and_exit() -> Result<()> {
    let exe = std::env::current_exe().context("current_exe")?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        Command::new(&exe)
            .args(&args)
            .creation_flags(DETACHED_PROCESS)
            .spawn()
            .with_context(|| format!("spawn {}", exe.display()))?;
    }
    #[cfg(not(windows))]
    {
        Command::new(&exe)
            .args(&args)
            .spawn()
            .with_context(|| format!("spawn {}", exe.display()))?;
    }

    std::process::exit(0);
}
