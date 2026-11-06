use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

/// Integer-friendly snapshot matching what the official app sends.
#[derive(Debug, Clone, Default)]
pub struct SensorSnapshot {
    pub cpu_temperature: f32,
    pub cpu_load: f32,
    pub cpu_clock: f32,
    pub cpu_power: f32,
    pub gpu_temperature: f32,
    pub gpu_load: f32,
    pub gpu_clock: f32,
    pub gpu_power: f32,
}

#[derive(Debug, Deserialize)]
struct BridgePayload {
    cpu_temperature: Option<f32>,
    cpu_load: Option<f32>,
    cpu_clock: Option<f32>,
    cpu_power: Option<f32>,
    gpu_temperature: Option<f32>,
    gpu_load: Option<f32>,
    gpu_clock: Option<f32>,
    gpu_power: Option<f32>,
}

pub struct SensorReader {
    system: System,
    shared: Arc<Mutex<SensorSnapshot>>,
    /// Keep the bridge process alive for the app lifetime.
    bridge: Option<Child>,
}

impl Drop for SensorReader {
    fn drop(&mut self) {
        if let Some(child) = self.bridge.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl SensorReader {
    pub fn new() -> Self {
        let refresh = RefreshKind::nothing().with_cpu(CpuRefreshKind::everything());
        let mut system = System::new_with_specifics(refresh).expect("init sysinfo System");
        system.refresh_specifics(refresh);

        let shared = Arc::new(Mutex::new(SensorSnapshot::default()));
        // Never start a second copy if one is already alive.
        let bridge = spawn_sensor_bridge_if_needed(Arc::clone(&shared));

        Self {
            system,
            shared,
            bridge,
        }
    }

    /// Kill only this reader's bridge child.
    pub fn stop_bridge(&mut self) {
        if let Some(mut child) = self.bridge.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Kill this bridge, then every `sensor-bridge.exe` (Quit completely).
    pub fn stop_bridge_and_sweep(&mut self) {
        self.stop_bridge();
        kill_all_sensor_bridges();
        // Confirm nothing is left.
        if is_sensor_bridge_running() {
            kill_all_sensor_bridges();
            thread::sleep(Duration::from_millis(150));
        }
    }

    /// Ensure we own a live bridge. Replaces any foreign/orphan instance.
    pub fn ensure_bridge(&mut self) {
        if self.own_bridge_alive() {
            return;
        }
        self.bridge = None;
        self.bridge = spawn_sensor_bridge_if_needed(Arc::clone(&self.shared));
    }

    fn own_bridge_alive(&mut self) -> bool {
        let Some(child) = self.bridge.as_mut() else {
            return false;
        };
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) | Err(_) => {
                self.bridge = None;
                false
            }
        }
    }

    pub fn poll(&mut self) -> SensorSnapshot {
        // Always refresh CPU load/clock from sysinfo as a baseline.
        self.system.refresh_cpu_all();
        let sys_load = self.system.global_cpu_usage();
        let sys_clock = self
            .system
            .cpus()
            .iter()
            .map(|c| c.frequency() as f32)
            .fold(0.0_f32, f32::max);

        let mut snap = self
            .shared
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        if snap.cpu_load <= 0.0 {
            snap.cpu_load = sys_load;
        }
        if snap.cpu_clock <= 0.0 {
            snap.cpu_clock = sys_clock;
        }
        snap
    }
}

/// True if any `sensor-bridge.exe` process is alive on this machine.
pub fn is_sensor_bridge_running() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq sensor-bridge.exe", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| {
                let text = String::from_utf8_lossy(&o.stdout).to_ascii_lowercase();
                text.contains("sensor-bridge.exe")
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Ensure exactly one bridge that *this* process owns (stdout pipe required).
///
/// A leftover non-admin `sensor-bridge` from an earlier launch cannot be reused —
/// kill it and start a fresh child instead of skipping (which left CPU temp at 0).
fn spawn_sensor_bridge_if_needed(shared: Arc<Mutex<SensorSnapshot>>) -> Option<Child> {
    if is_sensor_bridge_running() {
        tracing::info!(
            "replacing leftover sensor-bridge.exe so this process owns a single instance"
        );
        kill_all_sensor_bridges();
        thread::sleep(Duration::from_millis(250));
    }
    spawn_sensor_bridge(shared)
}

fn spawn_sensor_bridge(shared: Arc<Mutex<SensorSnapshot>>) -> Option<Child> {
    let Some(exe) = find_sensor_bridge() else {
        tracing::warn!(
            "sensor-bridge.exe not found — CPU temp/power need LibreHardwareMonitor bridge \
             (build sensor-bridge/ and keep it next to this exe, or under sensor-bridge/bin/Release/net8.0-windows/)"
        );
        return None;
    };

    tracing::info!(path = %exe.display(), "starting LibreHardwareMonitor sensor-bridge");
    let mut cmd = Command::new(&exe);
    cmd.current_dir(exe.parent().unwrap_or_else(|| Path::new(".")))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    // Prevent a console flash from the .NET helper (belt-and-suspenders with WinExe).
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(error = %err, "failed to start sensor-bridge");
            return None;
        }
    };

    let stdout = child.stdout.take()?;
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().flatten() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<BridgePayload>(line) {
                Ok(p) => {
                    if let Ok(mut g) = shared.lock() {
                        if let Some(v) = p.cpu_temperature {
                            g.cpu_temperature = v;
                        }
                        if let Some(v) = p.cpu_load {
                            g.cpu_load = v;
                        }
                        if let Some(v) = p.cpu_clock {
                            g.cpu_clock = v;
                        }
                        if let Some(v) = p.cpu_power {
                            g.cpu_power = v;
                        }
                        if let Some(v) = p.gpu_temperature {
                            g.gpu_temperature = v;
                        }
                        if let Some(v) = p.gpu_load {
                            g.gpu_load = v;
                        }
                        if let Some(v) = p.gpu_clock {
                            g.gpu_clock = v;
                        }
                        if let Some(v) = p.gpu_power {
                            g.gpu_power = v;
                        }
                    }
                }
                Err(err) => tracing::debug!(error = %err, line, "bad sensor-bridge JSON"),
            }
            thread::sleep(Duration::from_millis(10));
        }
        tracing::warn!("sensor-bridge stdout closed");
    });

    Some(child)
}

/// Force-kill every `sensor-bridge.exe` (including orphans from prior sessions).
pub fn kill_all_sensor_bridges() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/IM", "sensor-bridge.exe", "/F", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("pkill")
            .args(["-f", "sensor-bridge"])
            .output();
    }
}

fn find_sensor_bridge() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("sensor-bridge.exe"));
            candidates.push(dir.join("sensor-bridge").join("sensor-bridge.exe"));
        }
    }
    candidates.push(PathBuf::from("sensor-bridge.exe"));
    candidates.push(PathBuf::from(
        "sensor-bridge/bin/Release/net8.0-windows/sensor-bridge.exe",
    ));
    candidates.push(PathBuf::from(
        "sensor-bridge\\bin\\Release\\net8.0-windows\\sensor-bridge.exe",
    ));

    candidates.into_iter().find(|p| p.is_file())
}

impl SensorSnapshot {
    pub fn summary_line(&self) -> String {
        format!(
            "CPU {:>3.0}°C {:>3.0}% {:>4.0}MHz {:>3.0}W | GPU {:>3.0}°C {:>3.0}%",
            self.cpu_temperature,
            self.cpu_load,
            self.cpu_clock,
            self.cpu_power,
            self.gpu_temperature,
            self.gpu_load
        )
    }
}
