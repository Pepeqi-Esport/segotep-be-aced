//! Embed Inter as the app-wide egui typeface (replaces Ubuntu Light).

use std::sync::Arc;

use eframe::egui::{FontData, FontDefinitions, FontFamily};

const INTER: &str = "Inter";
const INTER_SEMIBOLD: &str = "Inter-SemiBold";

/// Install Inter for proportional + monospace text (emoji fonts remain as fallback).
pub fn install(ctx: &eframe::egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        INTER.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        INTER_SEMIBOLD.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-SemiBold.ttf"
        ))),
    );

    // Prefer Inter first so glyphs like `→` resolve; keep emoji fallbacks after.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(stack) = fonts.families.get_mut(&family) {
            stack.insert(0, INTER.to_owned());
            // SemiBold after Regular so missing Regular glyphs can still fall through.
            stack.insert(1, INTER_SEMIBOLD.to_owned());
        }
    }

    ctx.set_fonts(fonts);
}
