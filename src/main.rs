// Hide the Windows console for GUI launches (release). Debug builds keep a
// console for easier logging. CLI subcommands re-attach/allocate a console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod helpers;
mod services;
mod utils;
mod views;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use hidapi::HidApi;
use tracing_subscriber::EnvFilter;

use crate::config::AppConfig;
use crate::helpers::admin_helper;
use crate::helpers::dotnet_runtime_helper;
use crate::helpers::payload_helper;
use crate::services::background_service;
use crate::services::daemon_service::{run_daemon, send_screen_state, DaemonOptions};
use crate::services::device_service::{detect_default_mode, enumerate_devices};
use crate::services::protocol_service::{format_hex, DeviceMode};
use crate::services::sensor_service::SensorReader;
use crate::utils::constants::{LEGACY_PID, LEGACY_VID, SCREEN_PID, SCREEN_VID};

#[derive(Debug, Parser)]
#[command(
    name = "segotep-be-aced",
    about = "Segotep BeAced digital display — official-like GUI (Rust) with optional CLI"
)]
struct Cli {
    /// Path to config.json (defaults beside the exe)
    #[arg(long, global = true, default_value = "config.json")]
    config: PathBuf,

    /// Optional path to official/ for flashValue + config (read-only)
    #[arg(long, global = true, default_value = "official")]
    official: PathBuf,

    /// Start hidden in the system tray (used by login autostart)
    #[arg(long, global = true)]
    tray: bool,

    /// Force CLI help listing; without a subcommand the GUI opens
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Open the official-like GUI window
    Gui,
    /// Enumerate HID devices (highlights known BeAced IDs)
    List,
    /// Show matched BeAced device + flash profile
    Info {
        #[arg(long, value_enum)]
        mode: Option<ModeArg>,
    },
    /// Poll sensors and push values to the display (headless)
    Daemon {
        #[arg(long, value_enum)]
        mode: Option<ModeArg>,
        #[arg(long)]
        screen_off: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        once: bool,
        /// No console / no per-tick prints — used when the GUI hands off on close
        #[arg(long)]
        background: bool,
    },
    /// Toggle screen on/off (one-shot report)
    Screen {
        #[arg(value_enum)]
        state: ScreenState,
        #[arg(long, value_enum)]
        mode: Option<ModeArg>,
    },
    /// Print a sample packet for capture comparison
    Validate {
        #[arg(long, value_enum, default_value = "screen")]
        mode: ModeArg,
    },
    /// Write default config.json beside the exe if missing
    InitConfig,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ModeArg {
    Screen,
    Legacy,
}

impl From<ModeArg> for DeviceMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Screen => DeviceMode::Screen,
            ModeArg::Legacy => DeviceMode::Legacy,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScreenState {
    On,
    Off,
}

fn main() -> Result<()> {
    // Must run elevated (CPU sensors / HID). Show a clear popup if not.
    admin_helper::require_administrator();

    let cli = Cli::parse();
    let wants_gui = matches!(cli.command, None | Some(Commands::Gui));
    let background_daemon = matches!(
        cli.command,
        Some(Commands::Daemon {
            background: true,
            ..
        })
    );
    if !wants_gui && !background_daemon {
        attach_windows_console();
    }

    // GUI: quiet by default. CLI: show info logs on the attached console.
    let filter = if wants_gui {
        EnvFilter::from_default_env().add_directive("warn".parse()?)
    } else {
        EnvFilter::from_default_env().add_directive("info".parse()?)
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // sensor-bridge needs the .NET 8 Windows Desktop runtime.
    if let Err(err) = dotnet_runtime_helper::ensure_dotnet8_desktop() {
        tracing::error!(".NET 8 Desktop runtime setup failed: {err}");
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
            let msg = format!(
                "BeAced could not install .NET 8 Desktop Runtime:\n\n{err}\n\nInstall it from:\nhttps://dotnet.microsoft.com/download/dotnet/8.0"
            );
            let wide: Vec<u16> = std::ffi::OsStr::new(&msg)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let title: Vec<u16> = std::ffi::OsStr::new("BeAced")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            unsafe {
                let _ = MessageBoxW(None, PCWSTR(wide.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
            }
        }
    }

    // Single-exe UX: unpack sensor-bridge (+ deps) beside this binary.
    if let Err(err) = payload_helper::ensure_sidecars() {
        tracing::warn!("sidecar extract failed: {err}");
    }

    // Ensure config.json exists beside the exe (migrate legacy TOML / theme.json).
    let _ = crate::helpers::storage_helper::ensure_user_data()?;
    let config_path = if cli.config.is_absolute() {
        cli.config.clone()
    } else {
        payload_helper::config_path_beside_exe(&cli.config)
    };

    let official = if cli.official.exists() {
        Some(cli.official.clone())
    } else {
        None
    };
    let cfg = AppConfig::resolve(Some(config_path.clone()), official)?;

    match cli.command {
        None | Some(Commands::Gui) => {
            crate::views::run_gui(cfg, config_path, cli.tray)
                .map_err(|e| anyhow::anyhow!("GUI error: {e}"))?;
        }
        Some(Commands::List) => cmd_list()?,
        Some(Commands::Info { mode }) => cmd_info(&cfg, mode)?,
        Some(Commands::Daemon {
            mode,
            screen_off,
            dry_run,
            once,
            background: as_background,
        }) => {
            let api = HidApi::new()?;
            let mode = mode
                .map(DeviceMode::from)
                .unwrap_or_else(|| detect_default_mode(&api));
            if as_background {
                let _ = background_service::write_pid(std::process::id());
            }
            let result = run_daemon(
                &cfg,
                DaemonOptions {
                    mode,
                    screen_on: !screen_off,
                    dry_run,
                    once,
                    quiet: as_background,
                },
            );
            if as_background {
                background_service::clear_pid();
            }
            result?;
        }
        Some(Commands::Screen { state, mode }) => {
            let api = HidApi::new()?;
            let mode = mode
                .map(DeviceMode::from)
                .unwrap_or_else(|| detect_default_mode(&api));
            send_screen_state(&cfg, mode, matches!(state, ScreenState::On))?;
        }
        Some(Commands::Validate { mode }) => cmd_validate(&cfg, mode.into())?,
        Some(Commands::InitConfig) => {
            let (dir, _) = crate::helpers::storage_helper::ensure_user_data()?;
            println!(
                "ensured {}",
                crate::helpers::storage_helper::config_path(&dir).display()
            );
        }
    }

    Ok(())
}

/// Re-attach to the parent console (PowerShell/cmd) or allocate one for CLI use
/// when the binary is built with `windows_subsystem = "windows"`.
fn attach_windows_console() {
    #[cfg(windows)]
    {
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AttachConsole(dw_process_id: u32) -> i32;
            fn AllocConsole() -> i32;
            fn SetStdHandle(n_std_handle: u32, h_handle: *mut core::ffi::c_void) -> i32;
        }
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                let _ = AllocConsole();
            }
        }

        use std::fs::OpenOptions;
        use std::os::windows::io::{IntoRawHandle, OwnedHandle};

        const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6; // (u32)-10
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5; // (u32)-11
        const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4; // (u32)-12

        if let Ok(out) = OpenOptions::new().read(true).write(true).open("CONOUT$") {
            let raw = OwnedHandle::from(out).into_raw_handle();
            unsafe {
                SetStdHandle(STD_OUTPUT_HANDLE, raw);
                SetStdHandle(STD_ERROR_HANDLE, raw);
            }
        }
        if let Ok(inp) = OpenOptions::new().read(true).write(true).open("CONIN$") {
            let raw = OwnedHandle::from(inp).into_raw_handle();
            unsafe {
                SetStdHandle(STD_INPUT_HANDLE, raw);
            }
        }
    }
}

fn cmd_list() -> Result<()> {
    let api = HidApi::new()?;
    let devices = enumerate_devices(&api);
    println!(
        "{:<10} {:<10} {:<8} {}",
        "VID", "PID", "MODE", "PRODUCT / PATH"
    );
    for d in devices {
        let mode = match d.mode {
            Some(DeviceMode::Screen) => "screen",
            Some(DeviceMode::Legacy) => "legacy",
            None => "-",
        };
        let mark = if d.mode.is_some() { "*" } else { " " };
        println!(
            "{mark} {:#06X}   {:#06X}   {:<8} {} | {}",
            d.vendor_id,
            d.product_id,
            mode,
            d.product_string.unwrap_or_default(),
            d.path
        );
    }
    println!();
    println!("Known BeAced IDs:");
    println!(
        "  screen  VID={SCREEN_VID:#06X} PID={SCREEN_PID:#06X}  (official SendScreenValues)"
    );
    println!(
        "  legacy  VID={LEGACY_VID:#06X} PID={LEGACY_PID:#06X}  (official SendLatestValues)"
    );
    Ok(())
}

fn cmd_info(cfg: &AppConfig, mode: Option<ModeArg>) -> Result<()> {
    let api = HidApi::new()?;
    let mode = mode
        .map(DeviceMode::from)
        .unwrap_or_else(|| detect_default_mode(&api));
    let (vid, pid) = mode.vid_pid();
    let matched: Vec<_> = enumerate_devices(&api)
        .into_iter()
        .filter(|d| d.vendor_id == vid && d.product_id == pid)
        .collect();

    println!("mode:          {mode:?}");
    println!("expected VID:  {vid:#06X}");
    println!("expected PID:  {pid:#06X}");
    println!("flash product: {}", cfg.flash.product);
    println!("flash index:   {}", cfg.flash.index);
    println!("flash flags:   {:#010b}", cfg.flash.flags);
    println!("interval_ms:   {}", cfg.interval_ms);
    println!("matched HID:   {}", matched.len());
    for d in matched {
        println!("  - {} ({})", d.path, d.product_string.unwrap_or_default());
    }

    let mut sensors = SensorReader::new();
    let snap = sensors.poll();
    println!("sensors:       {}", snap.summary_line());
    Ok(())
}

fn cmd_validate(cfg: &AppConfig, mode: DeviceMode) -> Result<()> {
    let mut sensors = SensorReader::new();
    let snap = sensors.poll();
    let on = crate::services::protocol_service::build_report(mode, true, &cfg.flash, &snap, false);
    let off = crate::services::protocol_service::build_report(mode, false, &cfg.flash, &snap, false);

    println!("mode={mode:?}");
    println!("sensors: {}", snap.summary_line());
    println!("screen-on  ({} bytes): {}", on.len(), format_hex(&on));
    println!("screen-off ({} bytes): {}", off.len(), format_hex(&off));
    Ok(())
}
