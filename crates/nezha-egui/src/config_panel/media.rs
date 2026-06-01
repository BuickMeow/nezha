use crate::app::project_state::AudioStore;
use crate::app::project_state::MediaStore;
use crate::config_panel::ConfigAction;
use crate::transport::TimelineState;
use eframe::egui;
use nezha_media::MediaType;
use rust_i18n::t;

pub fn show(
    ui: &mut egui::Ui,
    media: &mut MediaStore,
    _timeline: &mut TimelineState,
    _audio: &mut AudioStore,
) -> Option<ConfigAction> {
    let mut action = None;

    ui.label(t!("media.library"));
    ui.add_space(4.0);

    if media.is_empty() {
        ui.label(t!("media.empty"));
    } else {
        for (idx, entry) in media.entries.iter().enumerate() {
            let info = &entry.info;
            let name = std::path::Path::new(&info.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&info.path);
            let icon = match info.media_type {
                MediaType::Video => "🎬",
                MediaType::Audio => "🔊",
                MediaType::Image => "🖼",
            };
            let duration_text = if info.media_type == MediaType::Image {
                rust_i18n::t!("media.image").to_string()
            } else {
                format!("{:.1}s", info.duration_secs)
            };

            ui.horizontal(|ui| {
                let btn_width = 52.0;
                let duration_width = 36.0;
                let icon_width = 20.0;
                let spacing = ui.spacing().item_spacing.x * 3.0;
                let reserved = btn_width + duration_width + icon_width + spacing;
                let available = ui.available_width() - reserved;
                let label = egui::Label::new(format!("{} {}", icon, name)).truncate();
                ui.add_sized(
                    [
                        available.max(20.0),
                        ui.text_style_height(&egui::TextStyle::Body),
                    ],
                    label,
                );
                ui.label(
                    egui::RichText::new(&duration_text)
                        .small()
                        .color(egui::Color32::GRAY),
                );
                if ui.button("➕").on_hover_text(t!("media.add_to_timeline")).clicked() {
                    action = Some(ConfigAction::AddMediaToTimeline(idx));
                }
                if ui.button("🗑").on_hover_text(t!("media.remove")).clicked() {
                    action = Some(ConfigAction::RemoveMedia(idx));
                }
            });
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.label(t!("media.import"));
    ui.add_space(4.0);

    if ui.button(format!("📥 {}", t!("media.import.btn"))).on_hover_text(t!("media.import.tooltip")).clicked() {
        action = Some(ConfigAction::ImportMedia);
    }

    action
}
