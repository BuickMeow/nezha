//! 属性面板入口。
//!
//! 根据选中 clip 的类型，委托给对应的子模块渲染属性 UI。
//!
//! 子模块位于 `properties_panel/` 目录（Rust 2018+ 约定）。

mod common;
mod counter;
mod solid_color;
mod waterfall;

use crate::app::project_state::MidiEntry;
use crate::transport::{ClipKind, TimelineState};
use common::show_common;
use eframe::egui;
use rust_i18n::t;

pub fn show(
    ui: &mut egui::Ui,
    timeline_state: &mut TimelineState,
    zoom: f32,
    midi_files: &[MidiEntry],
) {
    let fps = timeline_state.fps;
    egui::ScrollArea::vertical()
        .id_salt("properties_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading(t!("properties.title", zoom = (zoom * 100.0) as u32));
            ui.separator();

            let selected_count = timeline_state.selection.selected_count();
            let Some(selected_id) = timeline_state.selection.primary_selected() else {
                ui.label(t!("properties.none_selected"));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(t!("properties.none_selected.hint"))
                        .size(11.0)
                        .color(ui.visuals().weak_text_color()),
                );
                return;
            };

            // 多选时显示概要信息
            if selected_count > 1 {
                ui.label(egui::RichText::new(
                    t!("properties.multi_selected", count = selected_count),
                ).strong());
                ui.add_space(8.0);
                if ui
                    .button(
                        egui::RichText::new(format!("🗑 {}", t!("properties.delete_all")))
                            .color(egui::Color32::from_rgb(255, 120, 100)),
                    )
                    .clicked()
                {
                    timeline_state.remove_selected_clips();
                    return;
                }
                return;
            }

            // 查找选中的 clip
            let mut found = false;
            for track in &mut timeline_state.data.tracks {
                for clip in &mut track.clips {
                    if clip.id == selected_id {
                        found = true;

                        // 公共信息
                        ui.label(egui::RichText::new(&clip.name).strong());
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(t!("properties.start"));
                            ui.label(format!("{:.2}s", clip.start));
                        });
                        ui.horizontal(|ui| {
                            ui.label(t!("properties.end"));
                            ui.label(format!("{:.2}s", clip.end));
                        });
                        ui.horizontal(|ui| {
                            ui.label(t!("properties.duration"));
                            ui.label(format!("{:.2}s", clip.end - clip.start));
                        });

                        ui.add_space(8.0);

                        // 删除按钮
                        if ui
                            .button(
                                egui::RichText::new(format!("🗑 {}", t!("properties.delete")))
                                    .color(egui::Color32::from_rgb(255, 120, 100)),
                            )
                            .clicked()
                        {
                            timeline_state.remove_selected_clip();
                            return;
                        }

                        ui.separator();

                        // ── 通用属性（位置、缩放、合成方式、不透明度）──
                        show_common(ui, &mut clip.common);

                        ui.separator();

                        // ── 按类型委托特有属性 ──
                        match clip.kind {
                            ClipKind::Waterfall => {
                                waterfall::show(ui, clip, midi_files, fps);
                            }
                            ClipKind::SolidColor => {
                                solid_color::show(ui, clip);
                            }
                            ClipKind::Counter => {
                                counter::show(ui, clip, midi_files);
                            }
                            ClipKind::Audio => {
                                ui.label(t!("properties.layer.audio"));
                                if let Some(audio_idx) = clip.audio_idx {
                                    let msg = t!("properties.layer.audio_id", audio_idx = audio_idx);
                                    ui.label(msg);
                                }
                            }
                            ClipKind::Image => {
                                ui.label(t!("properties.layer.image"));
                                if let Some(media_idx) = clip.media_idx {
                                    let msg = t!("properties.layer.image_id", media_idx = media_idx);
                                    ui.label(msg);
                                }
                            }
                            ClipKind::Video => {
                                ui.label(t!("properties.layer.video"));
                                if let Some(media_idx) = clip.media_idx {
                                    let msg = t!("properties.layer.video_id", media_idx = media_idx);
                                    ui.label(msg);
                                }
                            }
                        }

                        break;
                    }
                }
                if found {
                    break;
                }
            }
        });
}
