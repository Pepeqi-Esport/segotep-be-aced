//! Herd-like palette and egui visual style.

use eframe::egui::{self, Color32, CornerRadius, Vec2};

pub const BG: Color32 = Color32::from_rgb(244, 244, 245);
pub const CARD: Color32 = Color32::WHITE;
pub const TEXT: Color32 = Color32::from_rgb(24, 24, 27);
pub const MUTED: Color32 = Color32::from_rgb(113, 113, 122);
pub const ACCENT: Color32 = Color32::from_rgb(225, 29, 72);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(255, 228, 230);
pub const BORDER: Color32 = Color32::from_rgb(228, 228, 231);
pub const OK: Color32 = Color32::from_rgb(22, 163, 74);
pub const WARN: Color32 = Color32::from_rgb(202, 138, 4);

pub fn apply_herd_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.visuals.window_fill = BG;
    style.visuals.panel_fill = BG;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(8);
    ctx.set_style(style);
}
