//! Launch-on-startup via Task Scheduler with highest privileges.
//!
//! A plain HKCU Run key starts the app unelevated → "must run as Administrator"
//! popup every login, and the display never gets sensor/HID access.
//! We use an AtLogOn task with RunLevel Highest instead (no UAC at login).

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, KEY_SET_VALUE, REG_SZ,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Value under `HKCU\...\Run` — leftover from older builds (cleanup only).
const RUN_VALUE: &str = "BeAced";
/// Task Scheduler name.
const TASK_NAME: &str = "segotep-be-aced_AutoStart";
/// Possible Windows service name (cleaned if present; we do not install one).
const SERVICE_NAME: &str = "segotep-be-aced";

pub fn is_enabled() -> bool {
    registry_enabled() || task_exists() || service_exists() || startup_shortcut_exists()
}

pub fn set_enabled(enable: bool) -> anyhow::Result<()> {
    // Always clear first so ON recreates cleanly and OFF leaves nothing behind.
    clear_all()?;
    if enable {
        create_elevated_logon_task()?;
    }
    Ok(())
}

fn create_elevated_logon_task() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let exe = exe.to_string_lossy().replace('\'', "''");
    let user = match (
        std::env::var("USERDOMAIN").ok(),
        std::env::var("USERNAME").ok(),
    ) {
        (Some(domain), Some(name)) if !domain.is_empty() => {
            format!("{domain}\\{name}").replace('\'', "''")
        }
        (_, Some(name)) => name.replace('\'', "''"),
        _ => anyhow::bail!("cannot resolve current Windows user for scheduled task"),
    };

    // Register-ScheduledTask is more reliable than `schtasks /RL HIGHEST /RU …`
    // (password / interactive-token edge cases). Must already be elevated.
    // Short delay gives USB HID + desktop a moment before we open the device.
    // `--tray` keeps the window hidden — only the tray icon + device updates.
    let ps = format!(
        "$ErrorActionPreference='Stop'; \
         $action = New-ScheduledTaskAction -Execute '{exe}' -Argument '--tray'; \
         $trigger = New-ScheduledTaskTrigger -AtLogOn -User '{user}'; \
         $trigger.Delay = 'PT10S'; \
         $principal = New-ScheduledTaskPrincipal -UserId '{user}' \
           -LogonType Interactive -RunLevel Highest; \
         $settings = New-ScheduledTaskSettingsSet \
           -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries \
           -ExecutionTimeLimit ([TimeSpan]::Zero) \
           -MultipleInstances IgnoreNew \
           -StartWhenAvailable; \
         Register-ScheduledTask -TaskName '{TASK_NAME}' -Action $action \
           -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null"
    );

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("failed to create elevated logon task: {err}{out}");
    }

    if !task_exists() {
        anyhow::bail!("elevated logon task was not created");
    }
    Ok(())
}

fn clear_all() -> anyhow::Result<()> {
    let _ = remove_registry_run();
    let _ = remove_scheduled_task();
    let _ = remove_service();
    let _ = remove_startup_shortcut();
    Ok(())
}

fn widestring(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// —— Registry (cleanup of older builds) ————————————————————————————————————

fn registry_enabled() -> bool {
    match open_run_key(KEY_READ) {
        Ok(key) => {
            let name = widestring(RUN_VALUE);
            let present = query_run_value(key, &name).is_some();
            unsafe {
                let _ = RegCloseKey(key);
            }
            present
        }
        Err(_) => false,
    }
}

fn remove_registry_run() -> anyhow::Result<()> {
    let key = match open_run_key(KEY_SET_VALUE) {
        Ok(k) => k,
        Err(_) => return Ok(()),
    };
    let names = [RUN_VALUE, "segotep-be-aced", "segotep-be-aced_AutoStart"];
    unsafe {
        for n in names {
            let wide = widestring(n);
            let _ = RegDeleteValueW(key, PCWSTR(wide.as_ptr()));
        }
        let _ = RegCloseKey(key);
    }
    Ok(())
}

fn open_run_key(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> anyhow::Result<HKEY> {
    let mut key = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            access,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        anyhow::bail!("RegOpenKeyExW(Run) failed: {:?}", status);
    }
    Ok(key)
}

fn query_run_value(key: HKEY, name: &[u16]) -> Option<String> {
    let mut ty = REG_SZ;
    let mut size = 0u32;
    let probe = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut ty),
            None,
            Some(&mut size),
        )
    };
    if probe != ERROR_SUCCESS || size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let u16s: Vec<u16> = buf
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let s = String::from_utf16_lossy(&u16s);
    let s = s.trim_end_matches('\0').to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

// —— Task Scheduler ———————————————————————————————————————————————————————

fn schtasks(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

fn task_exists() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn remove_scheduled_task() -> anyhow::Result<()> {
    // Prefer PowerShell so we can delete even if query naming differs slightly.
    let _ = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Unregister-ScheduledTask -TaskName '{TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue"
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = schtasks(&["/Delete", "/F", "/TN", TASK_NAME]);
    Ok(())
}

// —— Windows Service (cleanup only) ———————————————————————————————————————

fn sc(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("sc.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

fn service_exists() -> bool {
    sc(&["query", SERVICE_NAME])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn remove_service() -> anyhow::Result<()> {
    if !service_exists() {
        return Ok(());
    }
    let _ = sc(&["stop", SERVICE_NAME]);
    let _ = sc(&["delete", SERVICE_NAME])?;
    Ok(())
}

// —— Startup folder shortcut (cleanup) ————————————————————————————————————

fn startup_shortcut_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup")
            .join("BeAced.lnk"),
    )
}

fn startup_shortcut_exists() -> bool {
    startup_shortcut_path()
        .map(|p| p.is_file())
        .unwrap_or(false)
}

fn remove_startup_shortcut() -> anyhow::Result<()> {
    if let Some(path) = startup_shortcut_path() {
        if path.is_file() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
