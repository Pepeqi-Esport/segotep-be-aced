use eframe::egui;

use crate::views::components::widgets::{metric_row, section_card};
use crate::views::pages::HomePage;

pub fn metrics_panel(ui: &mut egui::Ui, page: &mut HomePage) {
    let p = page.palette();
    section_card(ui, &p, "Metrics", |ui| {
        for m in page.visible_metrics() {
            let selected = page.active == m;
            let (val, unit) = page.metric_value(m);
            if metric_row(ui, &p, m.group(), m.label(), selected, val, unit) {
                page.active = m;
            }
        }
    });
}
