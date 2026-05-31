use super::App;
use eframe::egui;

impl App {
    pub(super) fn render_preview(&mut self, ui: &mut egui::Ui) {
        if !self.export.has_export() {
            self.update_playback();
            let current_time = self.project.playback.current_time as f32;
            self.renderer
                .render_frame_for_export(current_time, &mut self.project);
        }

        let available = ui.available_size();
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        let aspect = render_width as f32 / render_height as f32;

        self.ui.zoom = crate::piano_view::show(
            ui,
            self.renderer.render_ctx.preview_texture_id(),
            available,
            aspect,
            &mut self.ui.zoom,
            &mut self.ui.pan_offset,
        );
    }
}
