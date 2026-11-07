use eframe::egui::{
    self, Align, CornerRadius, Frame, Layout, Margin, RichText, Sense, TextureHandle, Vec2,
};

use crate::views::pages::HomePage;

pub fn app_header(ui: &mut egui::Ui, page: &HomePage, logo: &TextureHandle) {
    let p = page.palette();
    ui.horizontal(|ui| {
        ui.add(
            egui::Image::from_texture(egui::load::SizedTexture::new(
                logo.id(),
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
                    .color(p.text),
            );
            ui.label(
                RichText::new("Digital display companion")
                    .size(13.0)
                    .color(p.muted),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let connected = page.device.is_some();
            let (dot, label) = if connected {
                (p.ok, "Connected")
            } else {
                (p.warn, "Device offline")
            };
            Frame::NONE
                .fill(if connected { p.ok_bg } else { p.warn_bg })
                .corner_radius(CornerRadius::same(255))
                .inner_margin(Margin::symmetric(10, 5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, dot);
                        ui.label(RichText::new(label).size(12.0).color(p.text));
                    });
                });
        });
    });
}
