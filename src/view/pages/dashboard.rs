//! Main control dashboard (metrics, settings, live reading, overview).

use eframe::egui::{self, Color32, RichText, Stroke};

use crate::helpers::autostart_helper;
use crate::utils::theme::Palette;
use crate::view::components::{
    card, header, metric_row, overview_line, theme_selector, toggle_row,
};
use super::BeAcedApp;
use crate::view::layouts::{app_shell, two_column};

pub struct DashboardPage;

impl DashboardPage {
    pub fn show(app: &mut BeAcedApp, ctx: &egui::Context) {
        let p = app.palette;
        let logo = app.logo.id();
        let connected = app.device.is_some();

        app_shell(ctx, &p, |ui| {
            header(ui, &p, logo, connected);
            ui.add_space(16.0);

            two_column(ui, |left, right| {
                left.vertical(|ui| {
                    Self::metrics_panel(app, ui, &p);
                    ui.add_space(12.0);
                    Self::settings_panel(app, ctx, ui, &p);
                });
                right.vertical(|ui| {
                    Self::live_reading_panel(app, ui, &p);
                    ui.add_space(12.0);
                    Self::overview_panel(app, ui, &p);
                    ui.add_space(12.0);
                    if !app.status.is_empty() {
                        ui.label(RichText::new(&app.status).size(12.0).color(p.muted));
                    }
                });
            });
        });
    }

    fn metrics_panel(app: &mut BeAcedApp, ui: &mut egui::Ui, p: &Palette) {
        card(ui, p, "Metrics", |ui| {
            for m in app.visible_metrics() {
                let selected = app.active == m;
                let (val, unit) = app.metric_value(m);
                if metric_row(ui, p, m, selected, val, unit) {
                    app.active = m;
                }
            }
        });
    }

    fn settings_panel(app: &mut BeAcedApp, ctx: &egui::Context, ui: &mut egui::Ui, p: &Palette) {
        card(ui, p, "Settings", |ui| {
            ui.set_min_height(220.0);
            let prev_screen = app.screen_on;
            toggle_row(ui, p, "Display on", &mut app.screen_on);
            if app.screen_on != prev_screen {
                app.push_hid();
            }

            ui.add_space(8.0);
            let mut auto = app.self_starting;
            let before = auto;
            toggle_row(ui, p, "Launch on startup", &mut auto);
            if auto != before {
                match autostart_helper::set_enabled(auto) {
                    Ok(()) => {
                        app.self_starting = auto;
                        app.status = if auto {
                            "Autostart enabled".into()
                        } else {
                            "Autostart disabled".into()
                        };
                    }
                    Err(err) => {
                        app.status = format!("Autostart failed: {err}");
                        app.self_starting = autostart_helper::is_enabled();
                    }
                }
            }

            ui.add_space(10.0);
            if let Some(theme) = theme_selector(ui, p, app.cfg.theme) {
                app.set_theme(ctx, theme);
            }

            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Close keeps the display updating in the tray. Click the tray icon to open again.",
                )
                .size(11.0)
                .color(p.muted),
            );
            ui.add_space(8.0);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Quit completely").size(13.0).color(p.accent),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0_f32, p.border)),
                )
                .clicked()
            {
                app.request_quit();
            }
        });
    }

    fn live_reading_panel(app: &BeAcedApp, ui: &mut egui::Ui, p: &Palette) {
        card(ui, p, "Live reading", |ui| {
            let (val, unit) = app.metric_value(app.active);
            ui.label(
                RichText::new(app.active.label())
                    .size(13.0)
                    .color(p.muted),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{val:.0}"))
                        .size(56.0)
                        .strong()
                        .color(p.text),
                );
                ui.label(RichText::new(unit).size(22.0).color(p.muted));
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{} · {}", app.active.group(), app.active.short()))
                    .size(12.0)
                    .color(p.muted),
            );
        });
    }

    fn overview_panel(app: &BeAcedApp, ui: &mut egui::Ui, p: &Palette) {
        card(ui, p, "Overview", |ui| {
            overview_line(
                ui,
                p,
                "CPU",
                &format!(
                    "{:.0}°C · {:.0}% · {:.0}W",
                    app.snap.cpu_temperature, app.snap.cpu_load, app.snap.cpu_power
                ),
            );
            overview_line(
                ui,
                p,
                "GPU",
                &format!(
                    "{:.0}°C · {:.0}% · {:.0}W",
                    app.snap.gpu_temperature, app.snap.gpu_load, app.snap.gpu_power
                ),
            );
            overview_line(
                ui,
                p,
                "Device",
                &format!("{:?} · {}", app.mode, app.cfg.flash.product),
            );
        });
    }
}
