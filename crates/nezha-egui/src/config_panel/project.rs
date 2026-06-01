//! 项目标签页 — 渲染设置 + 音色库管理。

use eframe::egui;
use rust_i18n::t;
use std::path::PathBuf;

use crate::app::project_state::{RenderSettings, SoundFontEntry};

pub fn show(
    ui: &mut egui::Ui,
    render_width: &mut u32,
    render_height: &mut u32,
    fps: &mut u32,
    soundfonts: &[SoundFontEntry],
) -> Option<ProjectAction> {
    let mut action = None;

    ui.label(t!("project.render_settings"));

    ui.horizontal(|ui| {
        ui.label(t!("project.resolution"));
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
        ui.label(t!("project.fps"));
        ui.add(egui::DragValue::new(fps).speed(1.0).range(1..=240));
        ui.label("fps");
    });

    ui.separator();
    ui.label(t!("project.soundfont"));
    ui.add_space(4.0);

    // SoundFont list
    if soundfonts.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(255, 180, 100),
            t!("project.soundfont.empty"),
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
                        ui.label(name.to_string());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🗑").clicked() {
                                action = Some(ProjectAction::RemoveSoundfont(i));
                            }
                            if i + 1 < soundfonts.len() && ui.button("↓").clicked() {
                                action = Some(ProjectAction::MoveSoundfontDown(i));
                            }
                            if i > 0 && ui.button("↑").clicked() {
                                action = Some(ProjectAction::MoveSoundfontUp(i));
                            }
                        });
                    });
                }
            });
    }

    if ui.button(t!("project.soundfont.add_btn")).clicked() {
        action = Some(ProjectAction::AddSoundfont(PathBuf::new()));
    }

    action
}

/// Audio render configuration dialog.
pub fn audio_render_dialog(
    ctx: &egui::Context,
    midi_name: &str,
    soundfonts: &[SoundFontEntry],
    render: &mut RenderSettings,
    open: &mut bool,
) -> Option<AudioRenderAction> {
    let mut action = None;
    let mut use_stereo = matches!(render.audio_channels, nezha_xsynth::ChannelCount::Stereo);

    egui::Window::new(t!("audio_render.title"))
        .open(open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label(format!("MIDI: {}", midi_name));

            ui.separator();
            ui.label(t!("audio_render.soundfont_list"));
            if soundfonts.is_empty() {
                ui.colored_label(egui::Color32::RED, t!("audio_render.soundfont.empty"));
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
                ui.label(t!("audio_render.sample_rate"));
                ui.add(
                    egui::DragValue::new(&mut render.audio_sample_rate)
                        .speed(100.0)
                        .range(8000..=192000),
                );
                ui.label("Hz");
            });

            ui.horizontal(|ui| {
                ui.label(t!("audio_render.channels"));
                ui.selectable_value(&mut use_stereo, true, t!("audio_render.channels.stereo"));
                ui.selectable_value(&mut use_stereo, false, t!("audio_render.channels.mono"));
            });

            ui.checkbox(&mut render.audio_use_limiter, t!("audio_render.limiter"));

            ui.horizontal(|ui| {
                ui.label(t!("audio_render.layers"));
                ui.add(
                    egui::DragValue::new(&mut render.audio_layers)
                        .speed(1.0)
                        .range(1..=256),
                );
            });

            ui.horizontal(|ui| {
                ui.label(t!("audio_render.min_velocity"));
                ui.add(egui::Slider::new(&mut render.audio_min_velocity, 0..=127).text(""));
            });
            ui.label(t!("audio_render.min_velocity.hint"));

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(t!("audio_render.cancel")).clicked() {
                    action = Some(AudioRenderAction::Cancel);
                }
                if ui
                    .button(egui::RichText::new(t!("audio_render.start")).strong())
                    .clicked()
                    && !soundfonts.is_empty()
                {
                    action = Some(AudioRenderAction::Start);
                }
            });
        });

    render.audio_channels = if use_stereo {
        nezha_xsynth::ChannelCount::Stereo
    } else {
        nezha_xsynth::ChannelCount::Mono
    };

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
    egui::Window::new(t!("audio_progress.title"))
        .open(open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label(t!("audio_progress.rendering"));
            ui.add_space(8.0);

            let progress_bar = egui::ProgressBar::new(progress as f32)
                .show_percentage()
                .desired_width(300.0);
            ui.add(progress_bar);

            ui.add_space(4.0);
            ui.label(t!("audio_progress.voices", count = current_voice));
            ui.label(t!("audio_progress.percent", percent = format!("{:.1}", progress * 100.0)));
        });
}

#[derive(Clone, Debug)]
#[expect(dead_code)]
pub enum ProjectAction {
    AddSoundfont(PathBuf),
    RemoveSoundfont(usize),
    MoveSoundfontUp(usize),
    MoveSoundfontDown(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioRenderAction {
    Start,
    Cancel,
}
