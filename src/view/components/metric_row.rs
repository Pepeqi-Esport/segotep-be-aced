use eframe::egui::{
    self, Align, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke,
};

use crate::utils::theme::Palette;
use crate::view::components::Metric;

/// Returns `true` if the row was clicked.
pub fn metric_row(
    ui: &mut egui::Ui,
    p: &Palette,
    metric: Metric,
    selected: bool,
    value: f32,
    unit: &str,
) -> bool {
    let row = Frame::NONE
        .fill(if selected { p.accent_soft } else { p.card })
        .stroke(Stroke::new(
            1.0_f32,
            if selected { p.accent } else { p.border },
        ))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(12, 10));
    let resp = row
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(metric.group())
                            .size(11.0)
                            .color(p.muted),
                    );
                    ui.label(
                        RichText::new(metric.label())
                            .size(14.0)
                            .strong()
                            .color(p.text),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{value:.0} {unit}"))
                            .size(16.0)
                            .strong()
                            .color(if selected { p.accent } else { p.text }),
                    );
                });
            });
        })
        .response
        .interact(Sense::click());
    ui.add_space(6.0);
    resp.clicked()
}
