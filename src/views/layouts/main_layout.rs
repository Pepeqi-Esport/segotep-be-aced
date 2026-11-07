use eframe::egui::{self, Frame, Margin};

use crate::views::components::{
    app_header, live_reading_panel, metrics_panel, overview_panel, settings_panel,
};
use crate::views::pages::HomePage;

pub fn main_layout(ctx: &egui::Context, page: &mut HomePage) {
    let p = page.palette();
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(p.bg).inner_margin(Margin::same(20)))
        .show(ctx, |ui| {
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
}
