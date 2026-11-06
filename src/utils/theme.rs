//! Herd-like palette and egui visual style (light / dark / system).

use eframe::egui::{self, Color32, CornerRadius, Theme, ThemePreference, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppTheme {
    Light,
    Dark,
    #[default]
    System,
}

impl AppTheme {
    pub const ALL: [AppTheme; 3] = [AppTheme::Light, AppTheme::Dark, AppTheme::System];

    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "System",
        }
    }

    pub fn to_egui(self) -> ThemePreference {
        match self {
            Self::Light => ThemePreference::Light,
            Self::Dark => ThemePreference::Dark,
            Self::System => ThemePreference::System,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub card: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub border: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub ok_bg: Color32,
    pub warn_bg: Color32,
    pub toggle_off: Color32,
    pub dark: bool,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            bg: Color32::from_rgb(244, 244, 245),
            card: Color32::WHITE,
            text: Color32::from_rgb(24, 24, 27),
            muted: Color32::from_rgb(113, 113, 122),
            accent: Color32::from_rgb(225, 29, 72),
            accent_soft: Color32::from_rgb(255, 228, 230),
            border: Color32::from_rgb(228, 228, 231),
            ok: Color32::from_rgb(22, 163, 74),
            warn: Color32::from_rgb(202, 138, 4),
            ok_bg: Color32::from_rgb(220, 252, 231),
            warn_bg: Color32::from_rgb(254, 249, 195),
            toggle_off: Color32::from_rgb(212, 212, 216),
            dark: false,
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(24, 24, 27),
            card: Color32::from_rgb(39, 39, 42),
            text: Color32::from_rgb(244, 244, 245),
            muted: Color32::from_rgb(161, 161, 170),
            accent: Color32::from_rgb(251, 113, 133),
            accent_soft: Color32::from_rgb(76, 29, 40),
            border: Color32::from_rgb(63, 63, 70),
            ok: Color32::from_rgb(74, 222, 128),
            warn: Color32::from_rgb(250, 204, 21),
            ok_bg: Color32::from_rgb(20, 83, 45),
            warn_bg: Color32::from_rgb(113, 63, 18),
            toggle_off: Color32::from_rgb(82, 82, 91),
            dark: true,
        }
    }

    pub fn resolve(preference: AppTheme, ctx: &egui::Context) -> Self {
        let theme = match preference {
            AppTheme::Light => Theme::Light,
            AppTheme::Dark => Theme::Dark,
            AppTheme::System => ctx
                .system_theme()
                .unwrap_or(Theme::Light),
        };
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }
}

/// Apply egui theme preference + our Herd palette styling.
pub fn apply_theme(ctx: &egui::Context, preference: AppTheme) -> Palette {
    ctx.set_theme(preference.to_egui());
    let palette = Palette::resolve(preference, ctx);
    apply_palette(ctx, &palette);
    palette
}

fn apply_palette(ctx: &egui::Context, palette: &Palette) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.visuals.dark_mode = palette.dark;
    style.visuals.window_fill = palette.bg;
    style.visuals.panel_fill = palette.bg;
    style.visuals.override_text_color = Some(palette.text);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.noninteractive.bg_stroke.color = palette.border;
    style.visuals.widgets.inactive.bg_stroke.color = palette.border;
    style.visuals.selection.bg_fill = palette.accent_soft;
    style.visuals.selection.stroke.color = palette.accent;
    ctx.set_style(style);
}
