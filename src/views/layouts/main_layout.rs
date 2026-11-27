use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, vec2};

use crate::views::components::{
    app_header, live_reading_panel, metrics_panel, overview_panel, settings_panel,
};
use crate::views::pages::HomePage;

pub fn main_layout(ui: &mut egui::Ui, page: &mut HomePage) {
    let p = page.palette();
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(p.bg).inner_margin(Margin::same(20)))
        .show(ui, |ui| {
            let logo = page.logo.clone();
            app_header(ui, page, &logo);
            ui.add_space(16.0);

            ui.columns(2, |cols| {
                let (left, right) = cols.split_at_mut(1);
                left[0].vertical(|ui| {
                    metrics_panel(ui, page);
                    ui.add_space(12.0);
                    settings_panel(ui, page);
                });
                right[0].vertical(|ui| {
                    live_reading_panel(ui, page);
                    ui.add_space(12.0);
                    overview_panel(ui, page);
                });
            });
        });

    if page.quit_fully {
        let p = page.palette();
        egui::Modal::new(egui::Id::unique("quit_modal"))
            .backdrop_color(Color32::from_black_alpha(160))
            .frame(
                Frame::NONE
                    .fill(p.card)
                    .stroke(Stroke::new(1.0_f32, p.border))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::symmetric(28, 24)),
            )
            .show(ui.ctx(), |ui| {
                ui.set_min_size(vec2(280.0, 110.0));
                ui.vertical_centered(|ui| {
                    ui.add(egui::Spinner::new().size(32.0).color(p.accent));
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("Shutting down…")
                            .size(16.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Stopping sensors and releasing drivers")
                            .size(12.0)
                            .color(p.muted),
                    );
                });
            });
        page.quit_ui_frames = page.quit_ui_frames.saturating_add(1);
        if page.quit_since.is_none() {
            page.quit_since = Some(std::time::Instant::now());
        }
    }
}
