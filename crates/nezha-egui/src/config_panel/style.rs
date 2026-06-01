//! 样式标签页 — MIDI 文件列表和添加图层。

use crate::app::project_state::MidiEntry;
use crate::config_panel::ConfigAction;
use eframe::egui;
use rust_i18n::t;

pub fn show(
    ui: &mut egui::Ui,
    midi_files: &[MidiEntry],
    highlighted_midi_idx: &mut Option<usize>,
) -> Option<ConfigAction> {
    let mut action = None;

    ui.label(t!("style.midi_files"));
    ui.add_space(4.0);

    if midi_files.is_empty() {
        ui.label(t!("style.midi_files.empty"));
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
                if ui.button("🎵").on_hover_text(t!("style.midi.render_audio")).clicked() {
                    action = Some(ConfigAction::RenderAudio(idx));
                }
                if ui.button("🗑").clicked() {
                    action = Some(ConfigAction::RemoveMidi(idx));
                }
            });
        }
    }

    ui.add_space(8.0);
    if ui.button(t!("style.midi.select")).clicked() {
        action = Some(ConfigAction::SelectMidi);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.label(t!("style.add_layers"));
    ui.add_space(4.0);

    if ui.button(t!("style.add_waterfall")).clicked() {
        action = Some(ConfigAction::AddWaterfall);
    }
    ui.add_space(4.0);
    if ui.button(t!("style.add_solid_color")).clicked() {
        action = Some(ConfigAction::AddSolidColor);
    }
    ui.add_space(4.0);
    if ui.button(t!("style.add_counter")).clicked() {
        action = Some(ConfigAction::AddCounter);
    }

    action
}
