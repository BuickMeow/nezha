//! 项目标签页 — 渲染设置 + 音色库管理。

use eframe::egui;
use std::path::PathBuf;

use crate::app::project_state::SoundFontEntry;

pub fn show(
    ui: &mut egui::Ui,
    render_width: &mut u32,
    render_height: &mut u32,
    fps: &mut u32,
    soundfonts: &[SoundFontEntry],
) -> Option<ProjectAction> {
    let mut action = None;

    ui.label("渲染设置");

    ui.horizontal(|ui| {
        ui.label("分辨率:");
        ui.add(
            egui::DragValue::new(render_width)
                .speed(1.0)
                .range(1..=7680),
        );
        ui.label("x");
        ui.add(
            egui::DragValue::new(render_height)
                .speed(1.0)
                .range(1..=4320),
        );
    });

    ui.horizontal(|ui| {
        ui.label("帧率:");
        ui.add(egui::DragValue::new(fps).speed(1.0).range(1..=240));
        ui.label("fps");
    });

    ui.separator();
    ui.label("音色库 (SoundFont)");
    ui.add_space(4.0);

    // SoundFont list
    if soundfonts.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(255, 180, 100),
            "尚未添加音色库。渲染音频需要至少一个 SF2/SFZ 文件。",
        );
    } else {
        let _total_height = soundfonts.len() as f32 * 24.0;
        egui::ScrollArea::vertical()
            .id_salt("soundfont_list")
            .max_height(150.0)
            .show(ui, |ui| {
                for (i, sf) in soundfonts.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let name = sf.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                        ui.label(format!("{}", name));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🗑").clicked() {
                                action = Some(ProjectAction::RemoveSoundfont(i));
                            }
                            if i + 1 < soundfonts.len() {
                                if ui.button("↓").clicked() {
                                    action = Some(ProjectAction::MoveSoundfontDown(i));
                                }
                            }
                            if i > 0 {
                                if ui.button("↑").clicked() {
                                    action = Some(ProjectAction::MoveSoundfontUp(i));
                                }
                            }
                        });
                    });
                }
            });
    }

    if ui.button("添加音色库...").clicked() {
        action = Some(ProjectAction::AddSoundfont(PathBuf::new()));
    }

    action
}

/// Audio render configuration dialog.
pub fn audio_render_dialog(
    ctx: &egui::Context,
    midi_name: &str,
    soundfonts: &[SoundFontEntry],
    sample_rate: &mut u32,
    use_stereo: &mut bool,
    use_limiter: &mut bool,
    layers: &mut u32,
    min_velocity: &mut u8,
    open: &mut bool,
) -> Option<AudioRenderAction> {
    let mut action = None;

    egui::Window::new("音频渲染")
        .open(open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label(format!("MIDI: {}", midi_name));

            ui.separator();
            ui.label("音色库列表:");
            if soundfonts.is_empty() {
                ui.colored_label(egui::Color32::RED, "⚠ 未选择音色库！");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for (i, sf) in soundfonts.iter().enumerate() {
                            let name = sf.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                            ui.label(format!("{}. {}", i + 1, name));
                        }
                    });
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("采样率:");
                ui.add(
                    egui::DragValue::new(sample_rate)
                        .speed(100.0)
                        .range(8000..=192000),
                );
                ui.label("Hz");
            });

            ui.horizontal(|ui| {
                ui.label("声道:");
                ui.selectable_value(use_stereo, true, "立体声");
                ui.selectable_value(use_stereo, false, "单声道");
            });

            ui.checkbox(use_limiter, "启用限制器");

            ui.horizontal(|ui| {
                ui.label("层数:");
                ui.add(egui::DragValue::new(layers).speed(1.0).range(1..=256));
            });

            ui.horizontal(|ui| {
                ui.label("最低力度阈值:");
                ui.add(egui::Slider::new(min_velocity, 0..=127).text(""));
            });
            ui.label("力度 ≤ 此值的音符将被筛除（默认 1 表示只筛除力度 0~1）");

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("取消").clicked() {
                    action = Some(AudioRenderAction::Start); // signal to close
                }
                if ui
                    .button(egui::RichText::new("开始渲染").strong())
                    .clicked()
                    && !soundfonts.is_empty()
                {
                    action = Some(AudioRenderAction::Start);
                }
            });
        });

    if action.is_some() {
        *open = false;
    }

    action
}

/// Rendering progress dialog.
pub fn audio_progress_dialog(
    ctx: &egui::Context,
    progress: f64,
    current_voice: u64,
    open: &mut bool,
) {
    egui::Window::new("音频渲染进度")
        .open(open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label("MIDI 音频渲染中...");
            ui.add_space(8.0);

            let progress_bar = egui::ProgressBar::new(progress as f32)
                .show_percentage()
                .desired_width(300.0);
            ui.add(progress_bar);

            ui.add_space(4.0);
            ui.label(format!("当前音色数: {}", current_voice));
            ui.label(format!("进度: {:.1}%", progress * 100.0));
        });
}

#[derive(Clone, Debug)]
pub enum ProjectAction {
    AddSoundfont(PathBuf),
    RemoveSoundfont(usize),
    MoveSoundfontUp(usize),
    MoveSoundfontDown(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioRenderAction {
    Start,
}
