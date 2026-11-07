use eframe::egui::{self, Align, Layout, RichText};

use crate::utils::theme::Palette;

pub fn overview_line(ui: &mut egui::Ui, p: &Palette, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(13.0).color(p.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(13.0).strong().color(p.text));
        });
    });
    ui.add_space(6.0);
}
