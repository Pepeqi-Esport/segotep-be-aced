//! Keep the cooler display alive after the GUI window closes by handing off
//! to a headless `daemon --background` process.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;

fn pid_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("segotep-be-aced").join("daemon.pid")
}

pub fn stop() {
    let path = pid_path();
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(pid) = text.trim().parse::<u32>() {
            kill_pid(pid);
        }
        let _ = fs::remove_file(&path);
    }

    // Also sweep orphaned `daemon --background` processes (stale pid file / older builds).
    #[cfg(windows)]
    {
        let self_pid = std::process::id();
        let script = format!(
            "Get-CimInstance Win32_Process -Filter \"Name='segotep-be-aced.exe'\" | \
             Where-Object {{ $_.ProcessId -ne {self_pid} -and $_.CommandLine -match 'daemon.*--background' }} | \
             ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }}"
        );
        let _ = Command::new("powershell.exe")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    // Brief pause so the HID handle is released before GUI reopens it.
    std::thread::sleep(std::time::Duration::from_millis(200));
}

fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
    }
}

pub fn start(screen_on: bool) -> anyhow::Result<()> {
    stop();

    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(&exe);
    cmd.arg("daemon").arg("--background");
    if !screen_on {
        cmd.arg("--screen-off");
    }

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }

    let child = cmd.spawn()?;
    write_pid(child.id())?;
    Ok(())
}

pub fn write_pid(pid: u32) -> anyhow::Result<()> {
    let path = pid_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, pid.to_string())?;
    Ok(())
}

pub fn clear_pid() {
    let _ = fs::remove_file(pid_path());
}
