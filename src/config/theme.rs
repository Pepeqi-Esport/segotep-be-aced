//! UI theme preference persisted as `theme.json` beside the exe.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::helpers::storage_helper;
use crate::utils::theme::AppTheme;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeConfig {
    /// light / dark / system
    pub theme: AppTheme,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            theme: AppTheme::System,
        }
    }
}

impl ThemeConfig {
    pub fn load(path: &Path) -> Result<Self> {
        storage_helper::read_json(path)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        storage_helper::write_json(path, self)
    }
}
