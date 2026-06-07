#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod model;
mod services;
mod theme;
mod view;

use anyhow::Result;

use crate::model::MaxionGuiApp;
use crate::theme::{configure_theme, install_fonts, APP_TITLE};

fn main() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([980.0, 700.0])
            .with_min_inner_size([820.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            configure_theme(&cc.egui_ctx);
            Ok(Box::new(MaxionGuiApp::new()))
        }),
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))
}
