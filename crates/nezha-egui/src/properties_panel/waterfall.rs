//! 瀑布流图层的属性面板。

use crate::app::project_state::MidiEntry;
use crate::config_panel::truncate_path;
use crate::transport::TrackClip;
use eframe::egui;
use nezha_renderer::RenderMode;

pub fn show(ui: &mut egui::Ui, clip: &mut TrackClip, midi_files: &[MidiEntry]) {
    ui.add_space(4.0);

    // MIDI 来源
    ui.label("MIDI 来源");

    let clip_id = clip.id;
    let current_name = clip
        .midi_idx
        .and_then(|idx| midi_files.get(idx))
        .and_then(|e| {
            std::path::Path::new(&e.path)
                .file_name()
                .and_then(|n| n.to_str())
        })
        .unwrap_or("（已删除）");
    let current_display = truncate_path(current_name, 18);

    egui::ComboBox::from_id_salt(format!("midi_source_{}", clip_id))
        .selected_text(current_display)
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
                }
            }
        });

    ui.add_space(4.0);

    // 渲染模式
    ui.label("渲染模式");
    let is_tick = clip.render_mode == RenderMode::TickBased;
    let mut mode_idx: usize = if is_tick { 1 } else { 0 };
    ui.horizontal(|ui| {
        ui.selectable_value(&mut mode_idx, 0, "秒模式");
        ui.selectable_value(&mut mode_idx, 1, "Tick 模式");
    });
    if mode_idx == 0 && is_tick {
        clip.render_mode = RenderMode::TimeBased;
    } else if mode_idx == 1 && !is_tick {
        clip.render_mode = RenderMode::TickBased;
    }

    ui.add_space(4.0);

    ui.label("钢琴键宽度");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut clip.equal_key_width, true, "等宽");
        ui.selectable_value(&mut clip.equal_key_width, false, "真实比例");
    });

    ui.add_space(4.0);

    ui.label("琴键区高度");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.keyboard_height_percent, 0.0..=0.5)
                .step_by(0.01)
                .text(""),
        );
    });
    ui.label(
        egui::RichText::new(format!(
            "{:.0}%（设为 0 则隐藏键盘）",
            clip.keyboard_height_percent * 100.0
        ))
        .size(11.0)
        .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(4.0);

    ui.label("流速");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.speed, 0.1..=100.0)
                .step_by(0.1)
                .text("x"),
        );
    });
    ui.label(
        egui::RichText::new(format!("当前: {:.1}x", clip.speed))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(8.0);
    ui.separator();
    ui.heading("瀑布流样式");
    ui.add_space(4.0);

    ui.label("边框宽度");
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
    ui.label("圆角");
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
