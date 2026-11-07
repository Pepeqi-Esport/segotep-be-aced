use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, Sense, Vec2};

use crate::utils::theme::Palette;

pub fn toggle_row(ui: &mut egui::Ui, p: &Palette, label: &str, on: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(14.0).color(p.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let desired = Vec2::new(44.0, 26.0);
            let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
            let t = if *on { 1.0 } else { 0.0 };
            let track = if *on { p.accent } else { p.toggle_off };
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
