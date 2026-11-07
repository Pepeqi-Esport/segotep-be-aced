use eframe::egui;

/// Equal two-column split used by the dashboard.
///
/// Both columns are handed to one closure so the caller can borrow app state
/// sequentially (left finishes before right starts).
pub fn two_column(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui, &mut egui::Ui)) {
    ui.columns(2, |cols| {
        let (left, right) = cols.split_at_mut(1);
        add_contents(&mut left[0], &mut right[0]);
    });
}
