//! 导出标签页 — 导出设置。

use nezha_encoder::EncoderBackend;
use rust_i18n::t;

use crate::app::project_state::MidiEntry;
use crate::config_panel::ConfigAction;
use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    export_format: &mut String,
    encoder: &mut String,
    encoder_backend: &mut String,
    export_path: &mut Option<String>,
    midi_files: &[MidiEntry],
) -> Option<ConfigAction> {
    let mut action = None;

    ui.label(t!("export.settings"));

    ui.horizontal(|ui| {
        ui.label(t!("export.format"));
        egui::ComboBox::from_id_salt("export_format")
            .selected_text(export_format.as_str())
            .show_ui(ui, |ui| {
                ui.selectable_value(export_format, "MP4".to_string(), "MP4");
                ui.selectable_value(export_format, "MOV".to_string(), "MOV");
                ui.selectable_value(export_format, "MKV".to_string(), "MKV");
                ui.selectable_value(export_format, "AVI".to_string(), "AVI");
            });
    });

    ui.horizontal(|ui| {
        ui.label(t!("export.encoder"));
        egui::ComboBox::from_id_salt("encoder")
            .selected_text(encoder.as_str())
            .show_ui(ui, |ui| {
                ui.selectable_value(encoder, "H.264".to_string(), "H.264");
                ui.selectable_value(encoder, "H.265 / HEVC".to_string(), "H.265 / HEVC");
                ui.selectable_value(encoder, "ProRes".to_string(), "ProRes");
                ui.selectable_value(encoder, "VP9".to_string(), "VP9");
                ui.selectable_value(encoder, "AV1".to_string(), "AV1");
            });
    });

    ui.horizontal(|ui| {
        ui.label(t!("export.acceleration"));
        let backends = EncoderBackend::available_on_current_platform();
        egui::ComboBox::from_id_salt("encoder_backend")
            .selected_text(encoder_backend.as_str())
            .show_ui(ui, |ui| {
                for b in &backends {
                    let name = b.display_name().to_string();
                    if ui
                        .selectable_value(encoder_backend, name.clone(), &name)
                        .clicked()
                    {
                        *encoder_backend = name;
                    }
                }
            });
    });

    ui.label(t!("export.location"));
    ui.horizontal(|ui| {
        if let Some(path) = export_path {
            let btn_width = 60.0;
            let spacing = ui.spacing().item_spacing.x;
            let available = (ui.available_width() - btn_width - spacing).max(40.0);
            ui.add_sized(
                [available, ui.text_style_height(&egui::TextStyle::Body)],
                egui::Label::new(path.as_str())
                    .truncate()
                    .sense(egui::Sense::hover()),
            )
            .on_hover_text(path.as_str());
        } else {
            ui.label(t!("export.location.none"));
        }
        if ui.button(t!("export.location.browse")).clicked() {
            let default_name = midi_files
                .first()
                .and_then(|entry| {
                    std::path::Path::new(&entry.path)
                        .file_stem()
                        .and_then(|n| n.to_str())
                })
                .unwrap_or("output");
            let ext = export_format.to_lowercase();
            let default_filename = format!("{}.{}", default_name, ext);

            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(&default_filename)
                .save_file()
            {
                let mut path_str = path.to_string_lossy().to_string();
                let expected_ext = format!(".{}", ext);
                if !path_str.to_lowercase().ends_with(&expected_ext) {
                    path_str.push_str(&expected_ext);
                }
                *export_path = Some(path_str);
            }
        }
    });

    ui.add_space(12.0);
    if ui.button(t!("export.start")).clicked() {
        action = Some(ConfigAction::StartExport);
    }

    action
}
