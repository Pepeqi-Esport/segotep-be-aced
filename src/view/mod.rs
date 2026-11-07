//! UI layer — React-style folders only:
//! - `components/` reusable widgets
//! - `layouts/` page shells / grids
//! - `pages/` screens + root `App`

pub mod components;
pub mod layouts;
pub mod pages;

pub use pages::run_gui;
