use super::App;
use eframe::egui;
use std::time::Instant;

impl App {
    pub(super) fn update_playback(&mut self) {
        if self.project.playback.is_playing {
            let now = Instant::now();
            let (start_instant, start_time) = self
                .project
                .playback
                .start
                .get_or_insert_with(|| (now, self.project.playback.current_time));
            let elapsed = now.duration_since(*start_instant).as_secs_f64();
            self.project.playback.current_time = (*start_time + elapsed).min(self.project.duration());

            if self.project.playback.current_time >= self.project.duration() {
                self.project.playback.current_time = 0.0;
                self.project.playback.is_playing = false;
                self.project.playback.start = None;
            }
        } else {
            self.project.playback.start = None;
        }
    }

    pub(super) fn handle_input(&mut self, ui: &mut egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::Space)) {
            self.project.playback.is_playing = !self.project.playback.is_playing;
            self.project.playback.start = None;
        }

        if !self.project.playback.is_playing {
            let frame_duration = 1.0 / self.project.render.fps.max(1) as f64;

            if ui.input(|input| input.key_pressed(egui::Key::ArrowLeft)) {
                self.project.playback.current_time =
                    (self.project.playback.current_time - frame_duration).max(0.0);
                self.project.playback.start = None;
            }

            if ui.input(|input| input.key_pressed(egui::Key::ArrowRight)) {
                self.project.playback.current_time =
                    (self.project.playback.current_time + frame_duration).min(self.project.duration());
                self.project.playback.start = None;
            }
        }

        let delete_pressed = ui.input(|input| {
            input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
        });
        if delete_pressed {
            self.project.timeline_state.remove_selected_clip();
        }
    }
}
