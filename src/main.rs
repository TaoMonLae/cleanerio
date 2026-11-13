mod app;
mod art;
mod linux;
mod pages;
mod scan;
mod threats;
mod ui_kit;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let shot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Cleanerio")
        .with_inner_size([1240.0, 780.0])
            .with_app_id("cleanerio")
        .with_min_inner_size([920.0, 620.0])
        // the app's own icon in the Dock and the app switcher
        .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")).expect("app icon"))
        .with_fullsize_content_view(true)
        .with_title_shown(false)
        .with_titlebar_shown(false);

    let mut options = eframe::NativeOptions::default();

    if shot.is_some() {
        // Open quietly behind the person's work: never steal focus.
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
        #[cfg(target_os = "macos")]
        {
            options.event_loop_builder = Some(Box::new(|builder| {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                builder.with_activate_ignoring_other_apps(false);
            }));
        }
    }
    options.viewport = viewport;

    eframe::run_native(
        "Cleanerio",
        options,
        Box::new(move |cc| Ok(Box::new(app::CleanerApp::new(cc, shot)))),
    )
}
