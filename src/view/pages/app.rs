//! Root app shell (React-style `App`) — state, lifecycle, window bootstrap.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use hidapi::{HidApi, HidDevice};

use crate::config::AppConfig;
use crate::helpers::autostart_helper;
use crate::helpers::logo_helper;
use crate::helpers::tray_helper::{self, AppTray, TrayAction};
use crate::services::background_service;
use crate::services::device_service::{detect_default_mode, open_be_aced, write_report};
use crate::services::protocol_service::{build_report, DeviceMode};
use crate::services::sensor_service::{SensorReader, SensorSnapshot};
use crate::utils::constants::{WINDOW_TITLE, WIN_H, WIN_MIN_H, WIN_MIN_W, WIN_W};
use crate::utils::theme::{apply_theme, AppTheme, Palette};
use crate::view::components::Metric;
use crate::view::pages::DashboardPage;

pub struct BeAcedApp {
    pub(crate) cfg: AppConfig,
    pub(crate) config_path: PathBuf,
    pub(crate) palette: Palette,
    /// Last resolved dark/light — used to react when OS theme changes under System.
    pub(crate) resolved_dark: bool,
    pub(crate) mode: DeviceMode,
    pub(crate) sensors: SensorReader,
    pub(crate) snap: SensorSnapshot,
    pub(crate) device: Option<HidDevice>,
    pub(crate) device_error: Option<String>,
    pub(crate) last_tick: Instant,
    pub(crate) interval: Duration,
    pub(crate) screen_on: bool,
    pub(crate) self_starting: bool,
    pub(crate) active: Metric,
    pub(crate) flash_flags: u8,
    pub(crate) status: String,
    /// When true, window close exits fully (leaves the tray).
    pub(crate) quit_fully: bool,
    /// Window is hidden; a headless daemon owns the HID device.
    pub(crate) tray_hidden: bool,
    pub(crate) tray: Option<AppTray>,
    pub(crate) logo: egui::TextureHandle,
    /// Autostart / `--tray`: hide to tray on the first frame.
    pub(crate) start_in_tray: bool,
}

impl BeAcedApp {
    pub fn new(
        cfg: AppConfig,
        config_path: PathBuf,
        tray: Option<AppTray>,
        logo: egui::TextureHandle,
        palette: Palette,
        start_in_tray: bool,
    ) -> Self {
        // Clean up any leftover headless daemon from older builds.
        // Keep it if we are about to hand off to tray immediately.
        if !start_in_tray {
            background_service::stop();
        }

        let api = HidApi::new().ok();
        let mode = api
            .as_ref()
            .map(detect_default_mode)
            .unwrap_or(DeviceMode::Screen);
        let mut sensors = SensorReader::new();
        let snap = sensors.poll();
        let flash_flags = cfg.flash.flags;
        let active = Metric::all()
            .into_iter()
            .find(|m| flash_flags & m.bit() != 0)
            .unwrap_or(Metric::CpuTemp);

        let (device, device_error) = match &api {
            Some(api) => match open_be_aced(api, mode, cfg.vendor_id, cfg.product_id) {
                Ok(d) => (Some(d), None),
                Err(e) => (None, Some(e.to_string())),
            },
            None => (None, Some("hidapi init failed".into())),
        };

        let interval = Duration::from_millis(cfg.interval_ms.max(200));
        let self_starting = autostart_helper::is_enabled();
        let resolved_dark = palette.dark;

        Self {
            cfg,
            config_path,
            palette,
            resolved_dark,
            mode,
            sensors,
            snap,
            device,
            device_error,
            last_tick: Instant::now() - interval,
            interval,
            screen_on: true,
            self_starting,
            active,
            flash_flags,
            status: String::new(),
            quit_fully: false,
            tray_hidden: false,
            tray,
            logo,
            start_in_tray,
        }
    }

    pub(crate) fn set_theme(&mut self, ctx: &egui::Context, theme: AppTheme) {
        self.cfg.theme = theme;
        self.palette = apply_theme(ctx, theme);
        self.resolved_dark = self.palette.dark;
        match self.cfg.save(&self.config_path) {
            Ok(()) => {
                self.status = format!("Theme: {}", theme.label());
            }
            Err(err) => {
                self.status = format!("Theme save failed: {err}");
            }
        }
    }

    fn sync_system_theme(&mut self, ctx: &egui::Context) {
        if self.cfg.theme != AppTheme::System {
            return;
        }
        let next = Palette::resolve(AppTheme::System, ctx);
        if next.dark != self.resolved_dark {
            self.palette = apply_theme(ctx, AppTheme::System);
            self.resolved_dark = self.palette.dark;
        }
    }

    fn reopen_hid(&mut self) {
        self.sensors.ensure_bridge();
        match HidApi::new() {
            Ok(api) => match open_be_aced(
                &api,
                self.mode,
                self.cfg.vendor_id,
                self.cfg.product_id,
            ) {
                Ok(d) => {
                    self.device = Some(d);
                    self.device_error = None;
                }
                Err(e) => {
                    self.device = None;
                    self.device_error = Some(e.to_string());
                }
            },
            Err(e) => {
                self.device = None;
                self.device_error = Some(e.to_string());
            }
        }
    }

    /// Hide to tray while keeping the cooler alive via headless daemon.
    fn handoff_to_tray(&mut self) {
        self.device = None;
        self.sensors.stop_bridge();
        if let Err(err) = background_service::start(self.screen_on) {
            self.status = format!("Tray handoff failed: {err}");
            self.sensors.ensure_bridge();
            self.reopen_hid();
            self.tray_hidden = false;
            tray_helper::show_window();
            return;
        }
        self.tray_hidden = true;
        tray_helper::hide_window();
        self.status = "Running in tray".into();
    }

    /// Take HID back from the background daemon after opening from tray.
    fn resume_from_tray(&mut self) {
        if !self.tray_hidden && self.device.is_some() {
            tray_helper::show_window();
            return;
        }
        background_service::stop();
        self.tray_hidden = false;
        self.reopen_hid();
        tray_helper::show_window();
        self.status = "Opened from tray".into();
    }

    pub(crate) fn visible_metrics(&self) -> Vec<Metric> {
        Metric::all()
            .into_iter()
            .filter(|m| self.flash_flags & m.bit() != 0)
            .collect()
    }

    pub(crate) fn push_hid(&mut self) {
        let report = build_report(
            self.mode,
            self.screen_on,
            &self.cfg.flash,
            &self.snap,
            false,
        );
        if let Some(dev) = &self.device {
            match write_report(dev, &report) {
                Ok(()) => {
                    self.device_error = None;
                    self.status = format!(
                        "Synced · {}",
                        crate::services::protocol_service::format_hex(&report)
                    );
                }
                Err(e) => {
                    self.status = format!("Write failed: {e}");
                    self.device_error = Some(e.to_string());
                }
            }
        } else if let Some(err) = &self.device_error {
            self.status = err.clone();
        }
    }

    /// Stop background work, blank the cooler, remove tray, then exit.
    ///
    /// Multiple live instances leave ghost/unclickable tray icons. We therefore
    /// NIM_DELETE our icon, then `taskkill` every BeAced/sensor-bridge process.
    pub(crate) fn request_quit(&mut self) {
        self.quit_fully = true;
        background_service::stop();
        self.screen_on = false;
        self.push_hid();
        self.device = None;
        // Own child + every leftover sensor-bridge.exe.
        self.sensors.stop_bridge_and_sweep();
        crate::services::sensor_service::kill_all_sensor_bridges();
        if let Some(tray_icon) = self.tray.take() {
            tray_helper::dispose(tray_icon);
        }
        // Terminates this process too — guarantees no leftover tray owner.
        tray_helper::kill_all_app_processes();
        std::process::exit(0);
    }

    fn tick(&mut self) {
        if self.quit_fully || self.tray_hidden {
            return;
        }
        if self.last_tick.elapsed() < self.interval {
            return;
        }
        self.last_tick = Instant::now();
        self.snap = self.sensors.poll();
        self.push_hid();
    }

    pub(crate) fn metric_value(&self, m: Metric) -> (f32, &'static str) {
        match m {
            Metric::CpuTemp => (self.snap.cpu_temperature, "°C"),
            Metric::CpuLoad => (self.snap.cpu_load, "%"),
            Metric::CpuClock => (self.snap.cpu_clock, "MHz"),
            Metric::CpuPower => (self.snap.cpu_power, "W"),
            Metric::GpuTemp => (self.snap.gpu_temperature, "°C"),
            Metric::GpuLoad => (self.snap.gpu_load, "%"),
            Metric::GpuClock => (self.snap.gpu_clock, "MHz"),
            Metric::GpuPower => (self.snap.gpu_power, "W"),
        }
    }
}

pub fn run_gui(cfg: AppConfig, config_path: PathBuf, start_in_tray: bool) -> eframe::Result<()> {
    // Prevent a second GUI (and a second dead tray icon) from stacking.
    if !tray_helper::try_become_singleton() {
        return Ok(());
    }

    // When opened normally, refresh the logon task so older installs pick up `--tray`.
    if !start_in_tray && autostart_helper::is_enabled() {
        let _ = autostart_helper::set_enabled(true);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WIN_W, WIN_H])
            .with_min_inner_size([WIN_MIN_W, WIN_MIN_H])
            .with_title(WINDOW_TITLE)
            .with_decorations(true)
            .with_visible(!start_in_tray)
            .with_icon(logo_helper::window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(move |cc| {
            let palette = apply_theme(&cc.egui_ctx, cfg.theme);
            let logo = logo_helper::load_texture(&cc.egui_ctx);
            let tray = match tray_helper::install(&cc.egui_ctx) {
                Ok(t) => Some(t),
                Err(err) => {
                    tracing::warn!("tray icon unavailable: {err}");
                    None
                }
            };
            Ok(Box::new(BeAcedApp::new(
                cfg,
                config_path,
                tray,
                logo,
                palette,
                start_in_tray,
            )))
        }),
    )
}

impl eframe::App for BeAcedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Login autostart (`--tray`): never show the window — tray + device only.
        if self.start_in_tray {
            self.start_in_tray = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            if self.tray.is_some() {
                self.handoff_to_tray();
            }
        }

        // Another exe launch (or tray left-click) asked us to come back.
        if tray_helper::take_external_show_request() {
            self.resume_from_tray();
        }

        if let Some(tray) = &self.tray {
            match tray_helper::poll_actions(tray) {
                Some(TrayAction::Show) => {
                    self.resume_from_tray();
                }
                Some(TrayAction::Quit) => {
                    self.request_quit();
                }
                None => {}
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quit_fully {
                self.request_quit();
            } else if self.tray.is_some() {
                // Herd-style: hide to tray; daemon keeps the device updating.
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.handoff_to_tray();
            } else {
                self.device = None;
                if let Err(err) = background_service::start(self.screen_on) {
                    self.status = format!("Background handoff failed: {err}");
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
        }

        self.tick();
        self.sync_system_theme(ctx);
        // Keep pumping while tray-hidden so external show requests are handled.
        ctx.request_repaint_after(Duration::from_millis(250));

        DashboardPage::show(self, ctx);
    }
}
