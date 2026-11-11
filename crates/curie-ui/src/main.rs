//! Curie-DAQ: Geiger-Müller counting with the PHYWE Cobra interface and serial pulse counters.
//!
//! A modern, general-purpose data-acquisition and analysis desktop application for nuclear physics
//! and radiation counting experiments, replacing legacy DOS software such as PHYWE RADIO1.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate alloc;

mod app;
mod device;
mod export;
mod modals;
mod pages;
mod plot_utils;
mod serial;
mod simulation;
#[cfg(test)]
mod tests;
mod theme;

use app::CurieApp;
use eframe::egui::ViewportBuilder;
use eframe::{NativeOptions, Result, run_native};

fn main() -> Result {
    let native_options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1100.0, 720.0])
            .with_title("Curie Lab - Nuclear Physics DAQ"),
        ..Default::default()
    };

    run_native(
        "Curie Lab",
        native_options,
        Box::new(|creation| {
            let mut app = CurieApp::new(creation);
            // Try the default port once; failures are only reported in 4:CONFIG.
            app.connect_serial();
            app.export_notice = None;
            app.load_saved_runs();
            Ok(Box::new(app))
        }),
    )
}
