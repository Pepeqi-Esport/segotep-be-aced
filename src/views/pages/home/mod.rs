//! Home page — owns UI state and implements `eframe::App` (like redis-manager).

mod actions;
mod theme;
mod update;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui;
use hidapi::{HidApi, HidDevice};

use crate::config::AppConfig;
use crate::helpers::autostart_helper;
use crate::helpers::logo_helper;
use crate::helpers::tray_helper::{self, AppTray, TrayAction};
use crate::helpers::update_helper::UpdatePrefs;
use crate::services::background_service;
use crate::services::device_service::{detect_default_mode, open_be_aced};
use crate::services::protocol_service::DeviceMode;
use crate::services::sensor_service::{SensorReader, SensorSnapshot};
use crate::utils::constants::{WINDOW_TITLE, WIN_H, WIN_MIN_H, WIN_MIN_W, WIN_W};
use crate::utils::theme::{apply_theme, AppTheme, Palette};
use crate::views::components::Metric;
use crate::views::layouts::main_layout;

use self::update::{load_prefs, update_channel, UpdateDialog, UpdateMsg};

pub struct HomePage {
    pub(crate) cfg: AppConfig,
    #[allow(dead_code)]
    pub(crate) config_path: PathBuf,
    pub(crate) theme_path: PathBuf,
    pub(crate) theme: AppTheme,
    pub(crate) palette: Palette,
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
    pub(crate) quit_fully: bool,
    pub(crate) tray_hidden: bool,
    pub(crate) tray: Option<AppTray>,
    pub(crate) logo: egui::TextureHandle,
    pub(crate) start_in_tray: bool,
    pub(crate) update_prefs: UpdatePrefs,
    pub(crate) update_dialog: Option<UpdateDialog>,
    pub(crate) update_busy: bool,
    pub(crate) update_startup_started: bool,
    pub(crate) update_tx: Sender<UpdateMsg>,
    pub(crate) update_rx: Receiver<UpdateMsg>,
}

impl HomePage {
    pub fn new(
        cfg: AppConfig,
        config_path: PathBuf,
        theme_path: PathBuf,
        theme: AppTheme,
        tray: Option<AppTray>,
        logo: egui::TextureHandle,
        palette: Palette,
        start_in_tray: bool,
    ) -> Self {
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
        let (update_tx, update_rx) = update_channel();

        Self {
            cfg,
            config_path,
            theme_path,
            theme,
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
            update_prefs: load_prefs(),
            update_dialog: None,
            update_busy: false,
            update_startup_started: false,
            update_tx,
            update_rx,
        }
    }

    pub fn palette(&self) -> Palette {
        self.palette
    }
}

pub fn run_gui(
    cfg: AppConfig,
    config_path: PathBuf,
    theme_path: PathBuf,
    theme: AppTheme,
    start_in_tray: bool,
) -> eframe::Result<()> {
    if !tray_helper::try_become_singleton() {
        return Ok(());
    }

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
            crate::utils::fonts::install(&cc.egui_ctx);
            let palette = apply_theme(&cc.egui_ctx, theme);
            let logo = logo_helper::load_texture(&cc.egui_ctx);
            let tray = match tray_helper::install(&cc.egui_ctx) {
                Ok(t) => Some(t),
                Err(err) => {
                    tracing::warn!("tray icon unavailable: {err}");
                    None
                }
            };
            Ok(Box::new(HomePage::new(
                cfg,
                config_path,
                theme_path,
                theme,
                tray,
                logo,
                palette,
                start_in_tray,
            )))
        }),
    )
}

impl eframe::App for HomePage {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.start_in_tray {
            self.start_in_tray = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            if self.tray.is_some() {
                self.handoff_to_tray();
            }
        }

        if tray_helper::take_external_show_request() {
            self.resume_from_tray();
        }

        if let Some(tray) = &self.tray {
            match tray_helper::poll_actions(tray) {
                Some(TrayAction::Show) => self.resume_from_tray(),
                Some(TrayAction::Quit) => self.request_quit(),
                None => {}
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quit_fully {
                self.request_quit();
            } else if self.tray.is_some() {
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

        self.ensure_startup_update_check(ctx);
        self.poll_update_messages();
        self.tick();
        self.sync_system_theme(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
        main_layout(ctx, self);
        self.show_update_dialog(ctx);
    }
}
