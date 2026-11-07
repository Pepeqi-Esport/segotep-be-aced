//! UI layer — same shape as redis-manager:
//! `pages` own state · `layouts` compose · `components` render.

pub mod components;
pub mod layouts;
pub mod pages;

pub use pages::run_gui;
#[allow(unused_imports)]
pub use pages::HomePage;
