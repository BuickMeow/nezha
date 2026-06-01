//! 瀑布流图层的属性面板。

use crate::app::project_state::MidiEntry;
use crate::transport::TrackClip;
use eframe::egui;
use nezha_renderer::RenderMode;
use rust_i18n::t;

pub fn show(ui: &mut egui::Ui, clip: &mut TrackClip, midi_files: &[MidiEntry], fps: u32) {
    ui.add_space(4.0);

    // MIDI 来源
    ui.label(t!("waterfall.midi_source"));

    let clip_id = clip.id;
    let deleted_label = t!("waterfall.midi_source.deleted").to_string();
    let current_name = clip
        .midi_idx
        .and_then(|idx| midi_files.get(idx))
        .and_then(|e| {
            std::path::Path::new(&e.path)
                .file_name()
                .and_then(|n| n.to_str())
        })
        .unwrap_or(&deleted_label);

    egui::ComboBox::from_id_salt(format!("midi_source_{}", clip_id))
        .selected_text(current_name)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (idx, entry) in midi_files.iter().enumerate() {
                let name = std::path::Path::new(&entry.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&entry.path);
                let selected = clip.midi_idx == Some(idx);
                if ui.selectable_label(selected, name).clicked() {
                    clip.midi_idx = Some(idx);
                    // 切换到新 MIDI 时自动更新 clip 长度和偏移
                    if let Some(entry) = midi_files.get(idx) {
                        let pre_song = crate::app::constants::DEFAULT_PRE_SONG_BUFFER;
                        clip.song_start_time = clip.start + pre_song;
                        clip.song_duration = entry.file.duration as f32;
                        clip.end = clip.song_start_time + clip.song_duration;
                        clip.update_content_offsets(fps);
                    }
                }
            }
        });

    ui.add_space(4.0);

    // 渲染模式
    ui.label(t!("waterfall.render_mode"));
    let is_tick = clip.render_mode == RenderMode::TickBased;
    let mut mode_idx: usize = if is_tick { 1 } else { 0 };
    ui.horizontal(|ui| {
        ui.selectable_value(&mut mode_idx, 0, t!("waterfall.render_mode.time"));
        ui.selectable_value(&mut mode_idx, 1, t!("waterfall.render_mode.tick"));
    });
    if mode_idx == 0 && is_tick {
        clip.render_mode = RenderMode::TimeBased;
    } else if mode_idx == 1 && !is_tick {
        clip.render_mode = RenderMode::TickBased;
    }

    ui.add_space(4.0);

    ui.label(t!("waterfall.key_width"));
    ui.horizontal(|ui| {
        ui.selectable_value(&mut clip.equal_key_width, true, t!("waterfall.key_width.equal"));
        ui.selectable_value(&mut clip.equal_key_width, false, t!("waterfall.key_width.real"));
    });

    ui.add_space(4.0);

    ui.label(t!("waterfall.keyboard_height"));
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.keyboard_height_percent, 0.0..=0.5)
                .step_by(0.01)
                .text(""),
        );
    });
    ui.label(
        egui::RichText::new(t!("waterfall.keyboard_height.hint", percent = (clip.keyboard_height_percent * 100.0) as u32))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(4.0);

    ui.label(t!("waterfall.speed"));
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.speed, 0.1..=100.0)
                .step_by(0.1)
                .text("x"),
        );
    });
    ui.label(
        egui::RichText::new(t!("waterfall.speed.current", speed = format!("{:.1}", clip.speed)))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(8.0);
    ui.separator();
    ui.heading(t!("waterfall.style"));
    ui.add_space(4.0);

    ui.label(t!("waterfall.border_width"));
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.border_width, 0.0..=1.0)
                .step_by(0.05)
                .text(""),
        );
    });
    ui.label(
        egui::RichText::new(format!("{:.0}%", clip.border_width * 100.0))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(4.0);
    ui.label(t!("waterfall.rounding"));
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.rounding, 0.0..=1.0)
                .step_by(0.05)
                .text(""),
        );
    });
    ui.label(
        egui::RichText::new(format!("{:.0}%", clip.rounding * 100.0))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );
}
