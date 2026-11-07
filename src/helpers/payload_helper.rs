//! Extract embedded sidecar files (sensor-bridge + deps) next to this exe when missing.
//!
//! Lets you ship a single `segotep-be-aced.exe`; first run writes helpers beside it.
//! Requires a Windows machine with the .NET 8 runtime for `sensor-bridge.exe`.

use std::fs;
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/embedded_payload.rs"));

/// Directory that should hold `sensor-bridge.exe` and friends (= folder of this exe).
pub fn install_dir() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow::anyhow!("cannot resolve install directory"))
}

/// Write any missing (or size-mismatched) embedded files next to the exe.
pub fn ensure_sidecars() -> anyhow::Result<()> {
    if EMBEDDED_FILES.is_empty() {
        tracing::warn!("no embedded sidecar payload — rebuild with sensor-bridge / .NET SDK");
        return Ok(());
    }

    let dir = install_dir()?;
    fs::create_dir_all(&dir)?;

    for &(name, bytes) in EMBEDDED_FILES {
        let path = dir.join(name);
        if path.is_file() {
            if let Ok(meta) = path.metadata() {
                if meta.len() == bytes.len() as u64 {
                    continue;
                }
            }
        }
        // .sys may be locked by a loaded kernel driver — soft-fail.
        if let Err(err) = fs::write(&path, bytes) {
            if name.ends_with(".sys") {
                tracing::warn!(file = name, error = %err, "could not write driver sidecar");
                continue;
            }
            anyhow::bail!("failed to extract {name}: {err}");
        }
        tracing::info!(file = name, bytes = bytes.len(), "extracted sidecar");
    }
    Ok(())
}

/// Prefer `config.json` (or the given relative name) next to the exe.
pub fn config_path_beside_exe(cli_path: &std::path::Path) -> PathBuf {
    if cli_path.is_absolute() {
        return cli_path.to_path_buf();
    }
    if let Ok(dir) = install_dir() {
        let beside = dir.join(cli_path);
        if beside.is_file() || !cli_path.exists() {
            return beside;
        }
    }
    cli_path.to_path_buf()
}
