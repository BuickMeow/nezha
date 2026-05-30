//! 样式标签页 — MIDI 文件列表和添加图层。

use crate::app::project_state::MidiEntry;
use crate::config_panel::ConfigAction;
use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    midi_files: &[MidiEntry],
    highlighted_midi_idx: &mut Option<usize>,
) -> Option<ConfigAction> {
    let mut action = None;

    ui.label("MIDI 文件");
    ui.add_space(4.0);

    if midi_files.is_empty() {
        ui.label("暂无 MIDI 文件");
    } else {
        for (idx, entry) in midi_files.iter().enumerate() {
            let is_highlighted = highlighted_midi_idx == &Some(idx);
            let raw_name = std::path::Path::new(&entry.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&entry.path);
            let full_path = &entry.path;
            let prefix = if is_highlighted { "▶ " } else { "  " };
            let text = format!("{}{}", prefix, raw_name);
            ui.horizontal(|ui| {
                let btn_width = 52.0;
                let spacing = ui.spacing().item_spacing.x;
                let available = (ui.available_width() - btn_width - spacing).max(20.0);
                let response = ui
                    .add_sized(
                        [available, ui.text_style_height(&egui::TextStyle::Body)],
                        egui::Label::new(text)
                            .truncate()
                            .selectable(false)
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_text(full_path);
                if response.clicked() {
                    *highlighted_midi_idx = Some(idx);
                }
                if ui.button("🎵").on_hover_text("渲染音频").clicked() {
                    action = Some(ConfigAction::RenderAudio(idx));
                }
                if ui.button("🗑").clicked() {
                    action = Some(ConfigAction::RemoveMidi(idx));
                }
            });
        }
    }

    ui.add_space(8.0);
    if ui.button("➕ 选择 MIDI / 压缩包 / DMS").clicked() {
        action = Some(ConfigAction::SelectMidi);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.label("添加图层到时间轴");
    ui.add_space(4.0);

    if ui.button("🌊 默认瀑布流").clicked() {
        action = Some(ConfigAction::AddWaterfall);
    }
    ui.add_space(4.0);
    if ui.button("🎨 纯色图层").clicked() {
        action = Some(ConfigAction::AddSolidColor);
    }
    ui.add_space(4.0);
    if ui.button("📊 音符计数器").clicked() {
        action = Some(ConfigAction::AddCounter);
    }

    action
}
