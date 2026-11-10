use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke,
    TextureHandle, Vec2,
};

use crate::helpers::update_helper;
use crate::views::pages::HomePage;

pub fn app_header(ui: &mut egui::Ui, page: &mut HomePage, logo: &TextureHandle) {
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

        // Fixed-size right cluster so the button always gets space (no RTL shrink).
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(120.0, 72.0),
                Layout::top_down(Align::RIGHT),
                |ui| {
                    let check_label = if page.update_busy {
                        "Checking…"
                    } else {
                        "Check update"
                    };
                    let check = ui.add_enabled(
                        !page.update_busy,
                        egui::Button::new(
                            RichText::new(check_label).size(12.0).color(Color32::WHITE),
                        )
                        .fill(p.accent)
                        .stroke(Stroke::new(1.0_f32, p.accent)),
                    );
                    if check.clicked() {
                        page.request_manual_update_check(ui.ctx());
                    }

                    ui.add_space(6.0);

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
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                                ui.painter().circle_filled(rect.center(), 4.0, dot);
                                ui.label(RichText::new(label).size(12.0).color(p.text));
                            });
                        });

                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!("v{}", update_helper::current_version()))
                            .size(11.0)
                            .color(p.muted),
                    );
                },
            );
        });
    });
}
