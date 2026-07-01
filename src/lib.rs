#![feature(portable_simd)]

pub const APP_NAME: &str = "Beatroot";
pub const IS_DEBUG: bool = cfg!(debug_assertions);

pub mod app;
pub mod internals;
pub mod project_manager;
pub mod ui;
