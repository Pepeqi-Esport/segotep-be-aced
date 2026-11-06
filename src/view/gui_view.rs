//! Modern Herd-inspired control window for BeAced.

use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke, Vec2,
};
use hidapi::{HidApi, HidDevice};

use crate::config::AppConfig;
use crate::helpers::autostart_helper;
use crate::helpers::logo_helper;
use crate::helpers::tray_helper::{self, AppTray, TrayAction};
use crate::services::background_service;
use crate::services::device_service::{detect_default_mode, open_be_aced, write_report};
use crate::services::protocol_service::{build_report, DeviceMode};
use crate::services::sensor_service::{SensorReader, SensorSnapshot};
use crate::utils::constants::{WINDOW_TITLE, WIN_H, WIN_W};
use crate::utils::theme::{
    apply_herd_style, ACCENT, ACCENT_SOFT, BG, BORDER, CARD, MUTED, OK, TEXT, WARN,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Metric {
    CpuTemp = 1,
    CpuLoad = 2,
    CpuClock = 3,
    CpuPower = 4,
    GpuTemp = 5,
    GpuLoad = 6,
    GpuClock = 7,
    GpuPower = 8,
}

impl Metric {
    fn all() -> [Metric; 8] {
        [
            Self::CpuTemp,
            Self::CpuLoad,
            Self::CpuClock,
            Self::CpuPower,
            Self::GpuTemp,
            Self::GpuLoad,
            Self::GpuClock,
            Self::GpuPower,
        ]
    }

    fn bit(self) -> u8 {
        1 << (self as u8 - 1)
    }

    fn label(self) -> &'static str {
        match self {
            Self::CpuTemp => "CPU Temperature",
            Self::CpuLoad => "CPU Load",
            Self::CpuClock => "CPU Clock",
            Self::CpuPower => "CPU Power",
            Self::GpuTemp => "GPU Temperature",
            Self::GpuLoad => "GPU Load",
            Self::GpuClock => "GPU Clock",
            Self::GpuPower => "GPU Power",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Self::CpuTemp => "Temp",
            Self::CpuLoad => "Load",
            Self::CpuClock => "Clock",
            Self::CpuPower => "Power",
            Self::GpuTemp => "Temp",
            Self::GpuLoad => "Load",
            Self::GpuClock => "Clock",
            Self::GpuPower => "Power",
        }
    }

    fn group(self) -> &'static str {
        match self {
            Self::CpuTemp | Self::CpuLoad | Self::CpuClock | Self::CpuPower => "CPU",
            _ => "GPU",
        }
    }
}

pub struct BeAcedApp {
    cfg: AppConfig,
    mode: DeviceMode,
    sensors: SensorReader,
    snap: SensorSnapshot,
    device: Option<HidDevice>,
    device_error: Option<String>,
    last_tick: Instant,
    interval: Duration,
    screen_on: bool,
    self_starting: bool,
    active: Metric,
    flash_flags: u8,
    status: String,
    /// When true, window close exits fully (leaves the tray).
    quit_fully: bool,
    /// Window is hidden; a headless daemon owns the HID device.
    tray_hidden: bool,
    tray: Option<AppTray>,
    logo: egui::TextureHandle,
}

impl BeAcedApp {
    pub fn new(cfg: AppConfig, tray: Option<AppTray>, logo: egui::TextureHandle) -> Self {
        // Clean up any leftover headless daemon from older builds.
        background_service::stop();

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

        Self {
            cfg,
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

    fn visible_metrics(&self) -> Vec<Metric> {
        Metric::all()
            .into_iter()
            .filter(|m| self.flash_flags & m.bit() != 0)
            .collect()
    }

    fn push_hid(&mut self) {
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
    fn request_quit(&mut self) {
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

    fn metric_value(&self, m: Metric) -> (f32, &'static str) {
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

pub fn run_gui(cfg: AppConfig) -> eframe::Result<()> {
    // Prevent a second GUI (and a second dead tray icon) from stacking.
    if !tray_helper::try_become_singleton() {
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WIN_W, WIN_H])
            .with_min_inner_size([680.0, 620.0])
            .with_title(WINDOW_TITLE)
            .with_decorations(true)
            .with_icon(logo_helper::window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|cc| {
            apply_herd_style(&cc.egui_ctx);
            let logo = logo_helper::load_texture(&cc.egui_ctx);
            let tray = match tray_helper::install(&cc.egui_ctx) {
                Ok(t) => Some(t),
                Err(err) => {
                    tracing::warn!("tray icon unavailable: {err}");
                    None
                }
            };
            Ok(Box::new(BeAcedApp::new(cfg, tray, logo)))
        }),
    )
}

impl eframe::App for BeAcedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        // Keep pumping while tray-hidden so external show requests are handled.
        ctx.request_repaint_after(Duration::from_millis(250));

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::same(20)))
            .show(ctx, |ui| {
                // Header — brand logo is the primary mark
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::from_texture(egui::load::SizedTexture::new(
                            self.logo.id(),
                            Vec2::splat(56.0),
                        ))
                        .sense(Sense::hover()),
                    );
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("BeAced")
                                .size(26.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new("Digital display companion")
                                .size(13.0)
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let connected = self.device.is_some();
                        let (dot, label) = if connected {
                            (OK, "Connected")
                        } else {
                            (WARN, "Device offline")
                        };
                        Frame::NONE
                            .fill(if connected {
                                Color32::from_rgb(220, 252, 231)
                            } else {
                                Color32::from_rgb(254, 249, 195)
                            })
                            .corner_radius(CornerRadius::same(255))
                            .inner_margin(Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) =
                                        ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 4.0, dot);
                                    ui.label(RichText::new(label).size(12.0).color(TEXT));
                                });
                            });
                    });
                });

                ui.add_space(16.0);

                ui.columns(2, |cols| {
                    // Left: metrics + settings
                    cols[0].vertical(|ui| {
                        card(ui, "Metrics", |ui| {
                            for m in self.visible_metrics() {
                                let selected = self.active == m;
                                let (val, unit) = self.metric_value(m);
                                let row = Frame::NONE
                                    .fill(if selected { ACCENT_SOFT } else { CARD })
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        if selected { ACCENT } else { BORDER },
                                    ))
                                    .corner_radius(CornerRadius::same(10))
                                    .inner_margin(Margin::symmetric(12, 10));
                                let resp = row
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(m.group())
                                                        .size(11.0)
                                                        .color(MUTED),
                                                );
                                                ui.label(
                                                    RichText::new(m.label())
                                                        .size(14.0)
                                                        .strong()
                                                        .color(TEXT),
                                                );
                                            });
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{val:.0} {unit}"
                                                        ))
                                                        .size(16.0)
                                                        .strong()
                                                        .color(if selected {
                                                            ACCENT
                                                        } else {
                                                            TEXT
                                                        }),
                                                    );
                                                },
                                            );
                                        });
                                    })
                                    .response
                                    .interact(Sense::click());
                                if resp.clicked() {
                                    self.active = m;
                                }
                                ui.add_space(6.0);
                            }
                        });

                        ui.add_space(12.0);

                        card(ui, "Settings", |ui| {
                            ui.set_min_height(140.0);
                            let prev_screen = self.screen_on;
                            toggle_row(ui, "Display on", &mut self.screen_on);
                            if self.screen_on != prev_screen {
                                self.push_hid();
                            }

                            ui.add_space(8.0);
                            let mut auto = self.self_starting;
                            let before = auto;
                            toggle_row(ui, "Launch on startup", &mut auto);
                            if auto != before {
                                match autostart_helper::set_enabled(auto) {
                                    Ok(()) => {
                                        self.self_starting = auto;
                                        self.status = if auto {
                                            "Autostart enabled".into()
                                        } else {
                                            "Autostart disabled".into()
                                        };
                                    }
                                    Err(err) => {
                                        self.status = format!("Autostart failed: {err}");
                                        self.self_starting = autostart_helper::is_enabled();
                                    }
                                }
                            }

                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(
                                    "Close keeps the display updating in the tray. Click the tray icon to open again.",
                                )
                                .size(11.0)
                                .color(MUTED),
                            );
                            ui.add_space(8.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Quit completely").size(13.0).color(ACCENT),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(Stroke::new(1.0_f32, BORDER)),
                                )
                                .clicked()
                            {
                                self.request_quit();
                            }
                        });
                    });

                    // Right: hero reading + status
                    cols[1].vertical(|ui| {
                        card(ui, "Live reading", |ui| {
                            let (val, unit) = self.metric_value(self.active);
                            ui.label(
                                RichText::new(self.active.label())
                                    .size(13.0)
                                    .color(MUTED),
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{val:.0}"))
                                        .size(56.0)
                                        .strong()
                                        .color(TEXT),
                                );
                                ui.label(
                                    RichText::new(unit).size(22.0).color(MUTED),
                                );
                            });
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!(
                                    "{} · {}",
                                    self.active.group(),
                                    self.active.short()
                                ))
                                .size(12.0)
                                .color(MUTED),
                            );
                        });

                        ui.add_space(12.0);

                        card(ui, "Overview", |ui| {
                            overview_line(ui, "CPU", &format!(
                                "{:.0}°C · {:.0}% · {:.0}W",
                                self.snap.cpu_temperature,
                                self.snap.cpu_load,
                                self.snap.cpu_power
                            ));
                            overview_line(ui, "GPU", &format!(
                                "{:.0}°C · {:.0}% · {:.0}W",
                                self.snap.gpu_temperature,
                                self.snap.gpu_load,
                                self.snap.gpu_power
                            ));
                            overview_line(
                                ui,
                                "Device",
                                &format!("{:?} · {}", self.mode, self.cfg.flash.product),
                            );
                        });

                        ui.add_space(12.0);

                        if !self.status.is_empty() {
                            ui.label(RichText::new(&self.status).size(12.0).color(MUTED));
                        }
                    });
                });
            });
    }
}

fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    Frame::NONE
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(12.0).strong().color(MUTED));
            ui.add_space(10.0);
            add_contents(ui);
        });
}

fn overview_line(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(13.0).color(MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(13.0).strong().color(TEXT));
        });
    });
    ui.add_space(6.0);
}

fn toggle_row(ui: &mut egui::Ui, label: &str, on: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(14.0).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let desired = Vec2::new(44.0, 26.0);
            let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
            let t = if *on { 1.0 } else { 0.0 };
            let track = if *on { ACCENT } else { Color32::from_rgb(212, 212, 216) };
            ui.painter()
                .rect_filled(rect, CornerRadius::same(255), track);
            let knob_r = 10.0;
            let x = egui::lerp((rect.left() + 13.0)..=(rect.right() - 13.0), t);
            ui.painter()
                .circle_filled(egui::pos2(x, rect.center().y), knob_r, Color32::WHITE);
            if resp.clicked() {
                *on = !*on;
            }
        });
    });
}
