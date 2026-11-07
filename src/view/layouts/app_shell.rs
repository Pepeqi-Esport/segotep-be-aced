use eframe::egui::{self, CentralPanel, Frame, Margin};

use crate::utils::theme::Palette;

/// Full-window panel with themed background and page padding.
pub fn app_shell(
    ctx: &egui::Context,
    p: &Palette,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    CentralPanel::default()
        .frame(Frame::NONE.fill(p.bg).inner_margin(Margin::same(20)))
        .show(ctx, |ui| add_contents(ui));
}
