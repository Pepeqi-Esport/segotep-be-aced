use eframe::egui::{self, Align, ComboBox, Layout, RichText};

use crate::utils::theme::{AppTheme, Palette};

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
