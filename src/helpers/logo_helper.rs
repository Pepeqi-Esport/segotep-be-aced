//! Shared BeAced brand logo (window icon, tray, in-app header).

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use image::imageops::FilterType;

const LOGO_PNG: &[u8] = include_bytes!("../../assets/images/logo/logo-256.png");

fn decode_rgba(size: u32) -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory(LOGO_PNG)
        .expect("logo-256.png")
        .into_rgba8();
    let resized = if img.width() == size && img.height() == size {
        img
    } else {
        image::imageops::resize(&img, size, size, FilterType::Lanczos3)
    };
    let (w, h) = resized.dimensions();
    (resized.into_raw(), w, h)
}

pub fn window_icon() -> egui::IconData {
    let (rgba, width, height) = decode_rgba(256);
    egui::IconData {
        rgba,
        width,
        height,
    }
}

pub fn tray_icon() -> tray_icon::Icon {
    let (rgba, width, height) = decode_rgba(32);
    tray_icon::Icon::from_rgba(rgba, width, height).expect("tray logo icon")
}

/// GPU texture for the in-app header (call once from `HomePage::new`).
pub fn load_texture(ctx: &egui::Context) -> TextureHandle {
    let (rgba, w, h) = decode_rgba(256);
    let color = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    ctx.load_texture("beaced_logo", color, TextureOptions::LINEAR)
}
