//! `cargo release` → dual-arch MSVC build + Inno Setup installer.
//! Invokes `scripts/build-installer.ps1` from the repo root.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest_dir.parent() else {
        eprintln!("xtask: cannot resolve repo root");
        return ExitCode::FAILURE;
    };
    let script = root.join("scripts").join("build-installer.ps1");
    if !script.is_file() {
        eprintln!("xtask: missing {}", script.display());
        return ExitCode::FAILURE;
    }

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script.to_str().unwrap_or_default(),
        ])
        .current_dir(root)
        .status();

    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(err) => {
            eprintln!("xtask: failed to start powershell: {err}");
            ExitCode::FAILURE
        }
    }
}
