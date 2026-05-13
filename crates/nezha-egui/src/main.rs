mod app;
mod config_panel;
mod piano_view;
mod properties_panel;
mod sidebar;
mod transport;

fn main() {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Nezha MIDI Renderer",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
    .unwrap();
}
