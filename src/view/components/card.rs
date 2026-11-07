use eframe::egui::{self, CornerRadius, Frame, Margin, RichText, Stroke};

use crate::utils::theme::Palette;

pub fn card(
    ui: &mut egui::Ui,
    p: &Palette,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    Frame::NONE
        .fill(p.card)
        .stroke(Stroke::new(1.0_f32, p.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(12.0).strong().color(p.muted));
            ui.add_space(10.0);
            add_contents(ui);
        });
}
