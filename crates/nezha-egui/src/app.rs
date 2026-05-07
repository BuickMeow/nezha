use std::time::Instant;
use eframe::egui;
use crate::sidebar;
use crate::config_panel;
use crate::properties_panel;
use crate::piano_view;
use crate::transport;

mod render_context;
mod project_state;
mod ui_state;

pub use render_context::RenderContext;
pub use project_state::ProjectState;
pub use ui_state::{UiState, ThemeMode};

pub struct App {
    pub render_ctx: RenderContext,
    pub project: ProjectState,
    pub ui: UiState,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "MiSans".to_owned(),
            egui::FontData::from_static(include_bytes!("../../../assets/MiSans-Regular.otf")).into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "MiSans".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let theme_mode = ThemeMode::System;
        theme_mode.apply(&cc.egui_ctx);

        Self {
            render_ctx: RenderContext::new(cc, 1920, 1080),
            project: ProjectState::new(),
            ui: UiState::default(),
        }
    }

    pub fn pick_midi_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MIDI", &["mid", "midi"])
            .pick_file()
        {
            self.project.load_midi(path.to_string_lossy().to_string(), &mut self.render_ctx);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui.theme_mode.apply(ui.ctx());

        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            self.project.is_playing = !self.project.is_playing;
            self.project.playback_start = None;
        }

        if !self.project.is_playing {
            let frame_duration = 1.0 / self.project.fps.max(1) as f64;
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                self.project.current_time = (self.project.current_time - frame_duration).max(0.0);
                self.project.playback_start = None;
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                self.project.current_time = (self.project.current_time + frame_duration)
                    .min(self.project.duration);
                self.project.playback_start = None;
            }
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let midi_path_clone = self.project.midi_path.clone();
            let mut config_action: Option<config_panel::ConfigAction> = None;

            // 1. 左侧导航栏
            egui::Panel::left("sidebar")
                .exact_size(60.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    sidebar::show(ui, &mut self.ui.active_tab);
                });

            // 2. 底部走带
            let dark_mode = self.ui.theme_mode.is_dark(ui.ctx());
            self.project.timeline_state.fps = self.project.fps;

            egui::Panel::bottom("transport")
                .exact_size(200.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    let mut transport_time = self.project.current_time as f32;
                    transport::show(
                        ui,
                        &mut self.project.is_playing,
                        &mut transport_time,
                        self.project.duration as f32,
                        &mut self.project.timeline_state,
                        dark_mode,
                    );
                    self.project.current_time = transport_time as f64;
                });

            // 3. 左侧面板 — 配置
            egui::Panel::left("config_panel")
                .exact_size(260.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    let mut state = config_panel::ConfigState {
                        active_tab: self.ui.active_tab,
                        midi_path: &midi_path_clone,
                        render_width: &mut self.project.render_width,
                        render_height: &mut self.project.render_height,
                        fps: &mut self.project.fps,
                        export_format: &mut self.ui.export_format,
                        encoder: &mut self.ui.encoder,
                        export_path: &mut self.ui.export_path,
                        theme_mode: &mut self.ui.theme_mode,
                    };
                    if let Some(action) = config_panel::show(ui, &mut state) {
                        config_action = Some(action);
                    }
                });

            match config_action {
                Some(config_panel::ConfigAction::SelectMidi) => self.pick_midi_file(),
                Some(config_panel::ConfigAction::Resize { width, height }) => {
                    self.render_ctx.resize(width, height);
                }
                Some(config_panel::ConfigAction::AddWaterfall) => {
                    let ts = &mut self.project.timeline_state;
                    let id = ts.next_clip_id;
                    ts.next_clip_id += 1;
                    let dur = self.project.duration as f32;
                    let mut new_track = crate::transport::Track::new_video(
                        &format!("视频 {}", ts.data.tracks.len() + 1)
                    );
                    new_track.clips.push(crate::transport::TrackClip {
                        id,
                        name: format!("瀑布流 {}", id),
                        kind: crate::transport::ClipKind::Waterfall,
                        start: 0.0,
                        end: dur.max(1.0),
                        color: egui::Color32::from_rgb(80, 150, 220),
                        speed: 1.0,
                        border_width: 0.1,
                        rounding: 0.0,
                    });
                    ts.data.tracks.push(new_track);
                }
                Some(config_panel::ConfigAction::AddSolidColor) => {
                    let ts = &mut self.project.timeline_state;
                    let id = ts.next_clip_id;
                    ts.next_clip_id += 1;
                    let dur = self.project.duration as f32;
                    let mut new_track = crate::transport::Track::new_video(
                        &format!("视频 {}", ts.data.tracks.len() + 1)
                    );
                    new_track.clips.push(crate::transport::TrackClip {
                        id,
                        name: format!("纯色 {}", id),
                        kind: crate::transport::ClipKind::SolidColor,
                        start: 0.0,
                        end: dur.max(1.0),
                        color: egui::Color32::from_rgb(200, 80, 80),
                        speed: 1.0,
                        border_width: 0.0,
                        rounding: 0.0,
                    });
                    ts.data.tracks.push(new_track);
                }
                None => {}
            }

            // 4. 右侧面板 — 属性
            egui::Panel::right("properties_panel")
                .exact_size(220.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    properties_panel::show(ui, &mut self.project.timeline_state, self.ui.zoom);
                });

            // 5. 中央预览区
            egui::CentralPanel::default().show_inside(ui, |ui| {
                if self.project.is_playing {
                    let now = Instant::now();
                    let (start_instant, start_time) = self.project.playback_start.get_or_insert_with(|| (now, self.project.current_time));
                    let elapsed = now.duration_since(*start_instant).as_secs_f64();
                    self.project.current_time = (*start_time + elapsed).min(self.project.duration);
                    if self.project.current_time >= self.project.duration {
                        self.project.current_time = 0.0;
                        self.project.is_playing = false;
                        self.project.playback_start = None;
                    }
                } else {
                    self.project.playback_start = None;
                }

                let available = ui.available_size();
                let rw = self.project.render_width as f32;
                let rh = self.project.render_height as f32;

                let render_time = self.project.current_time;

                let speed = self
                    .project
                    .timeline_state
                    .selected_clip_id
                    .and_then(|id| {
                        self.project
                            .timeline_state
                            .data
                            .tracks
                            .iter()
                            .flat_map(|t| t.clips.iter())
                            .find(|c| c.id == id)
                            .map(|c| c.speed)
                    })
                    .unwrap_or(1.0);

                let (selected_border, selected_rounding, selected_track) = self
                    .project
                    .timeline_state
                    .selected_clip_id
                    .and_then(|id| {
                        self.project
                            .timeline_state
                            .data
                            .tracks
                            .iter()
                            .flat_map(|t| t.clips.iter())
                            .find(|c| c.id == id)
                            .map(|c| (c.border_width, c.rounding, c.id))
                    })
                    .unwrap_or((0.1, 0.0, 0));

                let bg_color = self
                    .project
                    .timeline_state
                    .data
                    .tracks
                    .iter()
                    .flat_map(|t| t.clips.iter())
                    .find(|c| c.kind == crate::transport::ClipKind::SolidColor)
                    .map(|c| {
                        [
                            c.color.r() as f64 / 255.0,
                            c.color.g() as f64 / 255.0,
                            c.color.b() as f64 / 255.0,
                            1.0,
                        ]
                    })
                    .unwrap_or([0.0, 0.0, 0.0, 1.0]);

                let style = nezha_renderer::RenderStyle {
                    border_width: selected_border,
                    rounding: selected_rounding,
                    track_index: selected_track,
                    palette: nezha_renderer::random_palette(),
                    background: bg_color,
                };

                self.render_ctx.render(
                    self.project.render_width,
                    self.project.render_height,
                    render_time,
                    speed,
                    self.project.midi_file.as_ref().map(|m| m as &dyn nezha_renderer::NoteSource),
                    &style,
                );

                let aspect = rw / rh;
                self.ui.zoom = piano_view::show(
                    ui,
                    self.render_ctx.preview_texture_id,
                    available,
                    aspect,
                    &mut self.ui.zoom,
                    &mut self.ui.pan_offset,
                );
            });

            ui.ctx().request_repaint();
        });
    }
}
