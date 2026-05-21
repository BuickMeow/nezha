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

pub fn show(
    ui: &mut egui::Ui,
    timeline_state: &mut TimelineState,
    zoom: f32,
    midi_files: &[MidiEntry],
) {
    egui::ScrollArea::vertical()
        .id_salt("properties_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading(format!("属性（{:.0}%）", zoom * 100.0));
            ui.separator();

            let Some(selected_id) = timeline_state.selected_clip_id else {
                ui.label("未选中任何图层");
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("在时间轴上点击一个片段\n以编辑其属性")
                        .size(11.0)
                        .color(ui.visuals().weak_text_color()),
                );
                return;
            };

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
                            ui.label("开始:");
                            ui.label(format!("{:.2}s", clip.start));
                        });
                        ui.horizontal(|ui| {
                            ui.label("结束:");
                            ui.label(format!("{:.2}s", clip.end));
                        });
                        ui.horizontal(|ui| {
                            ui.label("时长:");
                            ui.label(format!("{:.2}s", clip.end - clip.start));
                        });

                        ui.add_space(8.0);

                        // 删除按钮
                        if ui
                            .button(
                                egui::RichText::new("🗑 删除此图层")
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
                                waterfall::show(ui, clip, midi_files);
                            }
                            ClipKind::SolidColor => {
                                solid_color::show(ui, clip);
                            }
                            ClipKind::Counter => {
                                counter::show(ui, clip);
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
