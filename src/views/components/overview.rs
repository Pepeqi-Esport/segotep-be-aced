use eframe::egui::{self, RichText};

use crate::views::components::widgets::{overview_line, section_card};
use crate::views::pages::HomePage;

pub fn overview_panel(ui: &mut egui::Ui, page: &HomePage) {
    let p = page.palette();
    section_card(ui, &p, "Overview", |ui| {
        overview_line(
            ui,
            &p,
            "CPU",
            &format!(
                "{:.0}°C · {:.0}% · {:.0}W",
                page.snap.cpu_temperature, page.snap.cpu_load, page.snap.cpu_power
            ),
        );
        overview_line(
            ui,
            &p,
            "GPU",
            &format!(
                "{:.0}°C · {:.0}% · {:.0}W",
                page.snap.gpu_temperature, page.snap.gpu_load, page.snap.gpu_power
            ),
        );
        overview_line(
            ui,
            &p,
            "Device",
            &format!("{:?} · {}", page.mode, page.cfg.flash.product),
        );
    });

    ui.add_space(12.0);
    if !page.status.is_empty() {
        ui.label(RichText::new(&page.status).size(12.0).color(p.muted));
    }
}
