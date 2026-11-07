use eframe::egui::{self, RichText};

use crate::views::components::widgets::section_card;
use crate::views::pages::HomePage;

pub fn live_reading_panel(ui: &mut egui::Ui, page: &HomePage) {
    let p = page.palette();
    section_card(ui, &p, "Live reading", |ui| {
        let (val, unit) = page.metric_value(page.active);
        ui.label(
            RichText::new(page.active.label())
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
            RichText::new(format!(
                "{} · {}",
                page.active.group(),
                page.active.short()
            ))
            .size(12.0)
            .color(p.muted),
        );
    });
}
