//! Shared UI primitives (page-agnostic).

use eframe::egui::{
    self, Align, Color32, ComboBox, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke,
    Vec2,
};

use crate::utils::theme::{AppTheme, Palette};

pub fn section_card(
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

pub fn overview_line(ui: &mut egui::Ui, p: &Palette, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(13.0).color(p.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(13.0).strong().color(p.text));
        });
    });
    ui.add_space(6.0);
}

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

/// Returns `true` if the row was clicked.
pub fn metric_row(
    ui: &mut egui::Ui,
    p: &Palette,
    group: &str,
    label: &str,
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
                    ui.label(RichText::new(group).size(11.0).color(p.muted));
                    ui.label(RichText::new(label).size(14.0).strong().color(p.text));
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

/// Returns the newly selected theme when it changes.
pub fn theme_selector(ui: &mut egui::Ui, p: &Palette, current: AppTheme) -> Option<AppTheme> {
    let mut selected = current;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Theme").size(14.0).color(p.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ComboBox::from_id_salt("theme_pref")
                .selected_text(selected.label())
                .width(110.0)
                .show_ui(ui, |ui| {
                    for t in AppTheme::ALL {
                        ui.selectable_value(&mut selected, t, t.label());
                    }
                });
        });
    });
    (selected != current).then_some(selected)
}
