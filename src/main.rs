use audio_cutter::audio_cutter_app;
use eframe::egui;
use std::sync::Arc;

fn main() -> eframe::Result {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../icon1100.png"))
        .expect("Application icon must be valid .png");

    let window_options = eframe::NativeOptions {
        // Viewport is an area in which the objects are going to be rendered (i.e. native window)
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([600.0, 300.0])
            .with_inner_size([600.0, 300.0])
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        "Audio Cutter",
        window_options,
        Box::new(|cc| Ok(Box::<audio_cutter_app::AudioCutterApp>::default())),
    )
}
