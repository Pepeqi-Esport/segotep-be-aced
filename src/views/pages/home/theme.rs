//! Theme persistence / system-theme sync for [`HomePage`].

use eframe::egui;

use crate::utils::theme::{apply_theme, AppTheme, Palette};
use crate::views::pages::HomePage;

impl HomePage {
    pub(crate) fn set_theme(&mut self, ctx: &egui::Context, theme: AppTheme) {
        self.theme = theme;
        self.cfg.theme = theme;
        self.palette = apply_theme(ctx, theme);
        self.resolved_dark = self.palette.dark;
        match self.cfg.save(&self.config_path) {
            Ok(()) => {
                self.status = format!("Theme: {}", theme.label());
            }
            Err(err) => {
                self.status = format!("Theme save failed: {err}");
            }
        }
    }

    pub(crate) fn sync_system_theme(&mut self, ctx: &egui::Context) {
        if self.theme != AppTheme::System {
            return;
        }
        let next = Palette::resolve(AppTheme::System, ctx);
        if next.dark != self.resolved_dark {
            self.palette = apply_theme(ctx, AppTheme::System);
            self.resolved_dark = self.palette.dark;
        }
    }
}
