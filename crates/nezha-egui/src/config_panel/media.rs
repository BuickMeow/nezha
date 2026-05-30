use crate::app::project_state::AudioStore;
use crate::app::project_state::MediaStore;
use crate::config_panel::ConfigAction;
use crate::transport::TimelineState;
use eframe::egui;
use nezha_media::MediaType;

pub fn show(
    ui: &mut egui::Ui,
    media: &mut MediaStore,
    _timeline: &mut TimelineState,
    _audio: &mut AudioStore,
) -> Option<ConfigAction> {
    let mut action = None;

    ui.label("素材库");
    ui.add_space(4.0);

    if media.is_empty() {
        ui.label("暂无素材");
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
                "图片".to_string()
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
                if ui.button("➕").on_hover_text("添加到时间线").clicked() {
                    action = Some(ConfigAction::AddMediaToTimeline(idx));
                }
                if ui.button("🗑").on_hover_text("移除素材").clicked() {
                    action = Some(ConfigAction::RemoveMedia(idx));
                }
            });
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.label("导入素材");
    ui.add_space(4.0);

    if ui.button("📥 导入媒体").on_hover_text("导入视频/音频/图片文件").clicked() {
        action = Some(ConfigAction::ImportMedia);
    }

    action
}
