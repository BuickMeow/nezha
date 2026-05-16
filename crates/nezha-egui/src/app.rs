use crate::config_panel;
use crate::piano_view;
use crate::properties_panel;
use crate::sidebar;
use crate::transport::{self, ClipKind, TrackClip};
use eframe::egui;
use std::time::Instant;

pub mod project_state;
mod render_context;
mod ui_state;

pub use project_state::ProjectState;
pub use render_context::RenderContext;
pub use ui_state::{ThemeMode, UiState};

pub struct App {
    pub render_ctx: RenderContext,
    pub project: ProjectState,
    pub ui: UiState,
}

/// 图层渲染所需数据（复制自 TrackClip，避免持有 self 的引用）
#[derive(Clone)]
struct LayerData {
    kind: ClipKind,
    midi_idx: Option<usize>,
    speed: f32,
    border_width: f32,
    rounding: f32,
    render_mode: nezha_renderer::RenderMode,
    equal_key_width: bool,
    clip_start: f32,
    color: egui::Color32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(feature = "profiling")]
        {
            puffin::set_scopes_on(true);
            // Leak the server so it lives for the entire app lifetime
            let _ = std::mem::ManuallyDrop::new(
                puffin_http::Server::new("0.0.0.0:8585").expect("puffin_http"),
            );
            println!("🔥 Puffin bridge on :8585 → puffin_viewer --url 127.0.0.1:8585");
        }

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "MiSans".to_owned(),
            egui::FontData::from_static(include_bytes!("../../../assets/MiSans-Regular.otf"))
                .into(),
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
            if self
                .project
                .load_midi(path.to_string_lossy().to_string())
                .is_ok()
            {
                self.render_ctx.reset_midi_state();
            }
        }
    }

    /// 收集当前时间点所有可见图层数据（Premiere 顺序：底→顶）
    fn collect_visible_layers(&self, time: f32) -> Vec<LayerData> {
        let mut layers: Vec<LayerData> = Vec::new();
        for track in self.project.timeline_state.data.tracks.iter().rev() {
            for clip in &track.clips {
                if time >= clip.start && time < clip.end {
                    layers.push(LayerData {
                        kind: clip.kind,
                        midi_idx: clip.midi_idx,
                        speed: clip.speed,
                        border_width: clip.border_width,
                        rounding: clip.rounding,
                        render_mode: clip.render_mode,
                        equal_key_width: clip.equal_key_width,
                        clip_start: clip.start,
                        color: clip.color,
                    });
                }
            }
        }
        layers
    }

    fn default_style(&self) -> nezha_renderer::RenderStyle {
        let ts = &self.project.timeline_state;
        let clip = ts.selected_clip();
        let (border_width, rounding, track_index, render_mode, equal_key_width, keyboard_percent) =
            clip.map(|c| {
                (
                    c.border_width,
                    c.rounding,
                    c.id,
                    c.render_mode,
                    c.equal_key_width,
                    c.keyboard_height_percent,
                )
            })
            .unwrap_or(TrackClip::default_render_params());

        let keyboard_height_px = self.project.render_height as f32 * keyboard_percent;

        nezha_renderer::RenderStyle {
            render_mode,
            border_width,
            rounding,
            track_index,
            palette: nezha_renderer::random_palette(),
            background: [0.0, 0.0, 0.0, 1.0],
            equal_key_width,
            keyboard_height: keyboard_height_px,
        }
    }

    fn handle_config_action(&mut self, action: config_panel::ConfigAction) {
        match action {
            config_panel::ConfigAction::SelectMidi => self.pick_midi_file(),
            config_panel::ConfigAction::Resize { width, height } => {
                self.render_ctx.resize(width, height);
            }
            config_panel::ConfigAction::AddWaterfall => {
                let dur = self.project.duration() as f32;
                let midi_idx = self.project.highlighted_midi_idx;
                self.project
                    .timeline_state
                    .push_waterfall_clip(midi_idx, dur);
            }
            config_panel::ConfigAction::AddSolidColor => {
                let dur = self.project.duration() as f32;
                let color = egui::Color32::from_rgb(200, 80, 80);
                self.project
                    .timeline_state
                    .push_solid_color_clip(color, dur);
            }
            config_panel::ConfigAction::RemoveMidi(idx) => {
                self.project.remove_midi(idx);
            }
        }
    }

    fn update_playback(&mut self) {
        if self.project.is_playing {
            let now = Instant::now();
            let (start_instant, start_time) = self
                .project
                .playback_start
                .get_or_insert_with(|| (now, self.project.current_time));
            let elapsed = now.duration_since(*start_instant).as_secs_f64();
            self.project.current_time = (*start_time + elapsed).min(self.project.duration());
            if self.project.current_time >= self.project.duration() {
                self.project.current_time = 0.0;
                self.project.is_playing = false;
                self.project.playback_start = None;
            }
        } else {
            self.project.playback_start = None;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "profiling")]
        puffin::GlobalProfiler::lock().new_frame();

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
                self.project.current_time =
                    (self.project.current_time + frame_duration).min(self.project.duration());
                self.project.playback_start = None;
            }
        }

        let delete_pressed =
            ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete_pressed {
            self.project.timeline_state.remove_selected_clip();
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
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

            let dur = self.project.duration() as f32;
            egui::Panel::bottom("transport")
                .exact_size(200.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    let mut transport_time = self.project.current_time as f32;
                    transport::show(
                        ui,
                        &mut self.project.is_playing,
                        &mut transport_time,
                        dur,
                        &mut self.project.timeline_state,
                        dark_mode,
                    );
                    self.project.current_time = transport_time as f64;
                });

            // 3. 配置面板
            egui::Panel::left("config_panel")
                .default_size(260.0)
                .min_size(180.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    let mut state = config_panel::ConfigState {
                        active_tab: self.ui.active_tab,
                        midi_files: &self.project.midi_files,
                        highlighted_midi_idx: &mut self.project.highlighted_midi_idx,
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

            if let Some(action) = config_action {
                self.handle_config_action(action);
            }

            // 4. 属性面板
            egui::Panel::right("properties_panel")
                .default_size(220.0)
                .min_size(160.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    properties_panel::show(
                        ui,
                        &mut self.project.timeline_state,
                        self.ui.zoom,
                        &self.project.midi_files,
                    );
                });

            // 5. 中央预览区 — 多图层叠加渲染
            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.update_playback();

                let available = ui.available_size();
                let rw = self.project.render_width as f32;
                let rh = self.project.render_height as f32;
                let current_time = self.project.current_time as f32;
                let render_w = self.project.render_width;
                let render_h = self.project.render_height;

                // 收集当前时间点所有可见图层（数据副本，不持有引用）
                let layers = self.collect_visible_layers(current_time);
                let default_style = self.default_style();

                // 纯色背景
                let bg = layers.iter().find(|c| c.kind == ClipKind::SolidColor);
                let bg_style = if let Some(bg) = bg {
                    nezha_renderer::RenderStyle {
                        background: [
                            bg.color.r() as f64 / 255.0,
                            bg.color.g() as f64 / 255.0,
                            bg.color.b() as f64 / 255.0,
                            1.0,
                        ],
                        ..default_style.clone()
                    }
                } else {
                    default_style.clone()
                };

                self.render_ctx.begin_pass();
                self.render_ctx
                    .render_background(render_w, render_h, &bg_style);

                // 瀑布流图层，从底到顶叠加
                let mut is_first_waterfall = true;
                for clip in &layers {
                    if clip.kind != ClipKind::Waterfall {
                        continue;
                    }
                    let Some(midi_idx) = clip.midi_idx else {
                        continue;
                    };
                    let Some(entry) = self.project.midi_files.get(midi_idx) else {
                        continue;
                    };

                    let clip_time = (current_time - clip.clip_start).max(0.0) as f64;

                    let clip_style = nezha_renderer::RenderStyle {
                        render_mode: clip.render_mode,
                        border_width: clip.border_width,
                        rounding: clip.rounding,
                        track_index: 0,
                        palette: default_style.palette,
                        background: default_style.background,
                        equal_key_width: clip.equal_key_width,
                        keyboard_height: default_style.keyboard_height,
                    };

                    self.render_ctx.render_layer(
                        render_w,
                        render_h,
                        clip_time,
                        clip.speed,
                        midi_idx,
                        &entry.file,
                        &clip_style,
                        is_first_waterfall && bg.is_none(),
                    );
                    is_first_waterfall = false;
                }
                self.render_ctx.end_pass();

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

        // 错误提示
        if let Some(err) = self.project.last_error.clone() {
            let mut dismissed = false;
            let screen_rect = ui.ctx().content_rect();
            egui::Area::new("error_toast".into())
                .fixed_pos(egui::pos2(screen_rect.center().x, 32.0))
                .anchor(egui::Align2::CENTER_TOP, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgb(60, 30, 30))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&err)
                                        .color(egui::Color32::from_rgb(255, 180, 100)),
                                );
                                if ui.button("✕").clicked() {
                                    dismissed = true;
                                }
                            });
                        });
                });
            if dismissed {
                self.project.last_error = None;
            }
        }
    }
}
