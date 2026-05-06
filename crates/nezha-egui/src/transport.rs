use eframe::egui;

pub fn show(ui: &mut egui::Ui, is_playing: &mut bool, current_time: &mut f32, duration: f32) {
    ui.horizontal(|ui| {
        ui.add_space(12.0);

        // 播放/暂停按钮
        let play_label = if *is_playing { "⏸ 暂停" } else { "▶ 播放" };
        if ui.button(play_label).clicked() {
            *is_playing = !*is_playing;
        }

        // 停止按钮
        if ui.button("⏹ 停止").clicked() {
            *is_playing = false;
            *current_time = 0.0;
        }

        ui.add_space(20.0);

        // 时间显示
        ui.label(format!("{:06.2} / {:06.2}", *current_time, duration));

        ui.add_space(12.0);

        // 进度条
        let progress = *current_time / duration;
        ui.add(egui::ProgressBar::new(progress).desired_width(400.0));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0);
            ui.label("Nezha MIDI Renderer");
        });
    });
}
