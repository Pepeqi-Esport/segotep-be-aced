//! Detect / install the .NET 8 **Windows Desktop** runtime required by sensor-bridge.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Official redirect to the latest .NET 8 Windows Desktop Runtime for this arch.
#[cfg(target_arch = "x86_64")]
const INSTALLER_URL: &str = "https://aka.ms/dotnet/8.0/windowsdesktop-runtime-win-x64.exe";
#[cfg(target_arch = "x86")]
const INSTALLER_URL: &str = "https://aka.ms/dotnet/8.0/windowsdesktop-runtime-win-x86.exe";

#[cfg(target_arch = "x86_64")]
const INSTALLER_FILE: &str = "windowsdesktop-runtime-8-win-x64.exe";
#[cfg(target_arch = "x86")]
const INSTALLER_FILE: &str = "windowsdesktop-runtime-8-win-x86.exe";

pub fn is_dotnet8_desktop_installed() -> bool {
    shared_framework_dirs().iter().any(|dir| {
        dir.join("Microsoft.WindowsDesktop.App")
            .read_dir()
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| {
                e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && e.file_name().to_string_lossy().starts_with("8.")
            })
    })
}

fn shared_framework_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        dirs.push(PathBuf::from(pf).join("dotnet").join("shared"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        dirs.push(PathBuf::from(pf86).join("dotnet").join("shared"));
    }
    // DOTNET_ROOT override
    if let Ok(root) = std::env::var("DOTNET_ROOT") {
        dirs.push(PathBuf::from(root).join("shared"));
    }
    dirs
}

/// If the Desktop runtime is missing, download and quiet-install it (needs admin + network).
pub fn ensure_dotnet8_desktop() -> anyhow::Result<()> {
    if is_dotnet8_desktop_installed() {
        tracing::info!(".NET 8 Windows Desktop runtime already installed");
        return Ok(());
    }

    tracing::warn!(".NET 8 Desktop runtime missing — downloading installer…");
    // No blocking dialog here: Startup launches must not wait on MessageBox.
    // Failures are reported by the caller (GUI MessageBox / log).

    let installer = download_installer()?;
    tracing::info!(path = %installer.display(), "running .NET 8 Desktop Runtime installer");

    let status = {
        let mut cmd = Command::new(&installer);
        cmd.args(["/install", "/quiet", "/norestart"]);
        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.status()
    }?;

    let _ = fs::remove_file(&installer);

    if !status.success() {
        anyhow::bail!(
            ".NET 8 installer exited with {status}. Install manually from https://dotnet.microsoft.com/download/dotnet/8.0"
        );
    }

    if !is_dotnet8_desktop_installed() {
        anyhow::bail!(
            ".NET installer finished but Microsoft.WindowsDesktop.App 8.x was not found. A reboot may be required."
        );
    }

    tracing::info!(".NET 8 Windows Desktop runtime installed successfully");
    Ok(())
}

fn download_installer() -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join("segotep-be-aced");
    fs::create_dir_all(&dir)?;
    let out = dir.join(INSTALLER_FILE);
    if out.is_file() {
        let _ = fs::remove_file(&out);
    }

    // curl.exe ships with Windows 10+ and follows aka.ms redirects.
    let mut cmd = Command::new("curl.exe");
    cmd.args([
        "-fsSL",
        "--retry",
        "3",
        "-o",
        out.to_str().unwrap_or("dotnet8-desktop.exe"),
        INSTALLER_URL,
    ]);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd.status();
    match status {
        Ok(s) if s.success() && out.is_file() => Ok(out),
        Ok(s) => anyhow::bail!("curl download failed ({s})"),
        Err(err) => {
            // Fallback: PowerShell
            let ps = format!(
                "Invoke-WebRequest -UseBasicParsing -Uri '{}' -OutFile '{}'",
                INSTALLER_URL,
                out.display()
            );
            let mut pscmd = Command::new("powershell.exe");
            pscmd.args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &ps,
            ]);
            #[cfg(windows)]
            {
                pscmd.creation_flags(CREATE_NO_WINDOW);
            }
            let st = pscmd.status().map_err(|e| {
                anyhow::anyhow!("download failed (curl: {err}; powershell: {e})")
            })?;
            if !st.success() || !out.is_file() {
                anyhow::bail!("failed to download .NET 8 Desktop Runtime installer");
            }
            Ok(out)
        }
    }
}
