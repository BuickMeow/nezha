use crate::app::project_state::MidiEntry;
use crate::config_panel::truncate_str;
use crate::transport::{ClipKind, TimelineState};
use eframe::egui;
use nezha_renderer::RenderMode;

pub fn show(
    ui: &mut egui::Ui,
    timeline_state: &mut TimelineState,
    zoom: f32,
    midi_files: &[MidiEntry],
) {
    ui.heading(format!("属性（{:.0}%）", zoom * 100.0));
    ui.separator();

    if let Some(selected_id) = timeline_state.selected_clip_id {
        let mut found = false;
        for track in &mut timeline_state.data.tracks {
            for clip in &mut track.clips {
                if clip.id == selected_id {
                    found = true;

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

                    match clip.kind {
                        ClipKind::Waterfall => {
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
                            let current_display = truncate_str(current_name, 20);

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
                        ClipKind::SolidColor => {
                            ui.add_space(4.0);
                            ui.label("颜色");
                            let mut rgb = [clip.color.r(), clip.color.g(), clip.color.b()];
                            ui.color_edit_button_srgb(&mut rgb);
                            clip.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                        }
                    }

                    break;
                }
            }
            if found {
                break;
            }
        }
    } else {
        ui.label("未选中任何图层");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("在时间轴上点击一个片段\n以编辑其属性")
                .size(11.0)
                .color(ui.visuals().weak_text_color()),
        );
    }
}
