use eframe::egui::{self, Color32, RichText, Stroke};

use crate::helpers::autostart_helper;
use crate::views::components::widgets::{section_card, theme_selector, toggle_row};
use crate::views::pages::HomePage;

pub fn settings_panel(ui: &mut egui::Ui, page: &mut HomePage) {
    let p = page.palette();
    section_card(ui, &p, "Settings", |ui| {
        // Metrics above can leave little vertical room — keep Settings scrollable.
        egui::ScrollArea::vertical()
            .max_height(280.0)
            .show(ui, |ui| {
                let prev_screen = page.screen_on;
                toggle_row(ui, &p, "Display on", &mut page.screen_on);
                if page.screen_on != prev_screen {
                    page.push_hid();
                }

                ui.add_space(8.0);
                let mut auto = page.self_starting;
                let before = auto;
                toggle_row(ui, &p, "Launch on startup", &mut auto);
                if auto != before {
                    match autostart_helper::set_enabled(auto) {
                        Ok(()) => {
                            page.self_starting = auto;
                            page.status = if auto {
                                "Autostart enabled".into()
                            } else {
                                "Autostart disabled".into()
                            };
                        }
                        Err(err) => {
                            page.status = format!("Autostart failed: {err}");
                            page.self_starting = autostart_helper::is_enabled();
                        }
                    }
                }

                ui.add_space(10.0);
                if let Some(theme) = theme_selector(ui, &p, page.theme) {
                    page.set_theme(ui.ctx(), theme);
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
                let quitting = page.quit_fully;
                let label = if quitting {
                    "Shutting down…"
                } else {
                    "Quit completely"
                };
                if ui
                    .add_enabled(
                        !quitting,
                        egui::Button::new(RichText::new(label).size(13.0).color(p.accent))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::new(1.0_f32, p.border)),
                    )
                    .clicked()
                {
                    page.begin_quit();
                }
            });
    });
}
