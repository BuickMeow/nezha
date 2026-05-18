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

use std::sync::mpsc;

enum MidiLoadEvent {
    Progress(nezha_core::LoadProgress),
    Complete(Result<nezha_core::MidiFile, nezha_core::MidiError>),
}

struct MidiLoader {
    path: String,
    rx: mpsc::Receiver<MidiLoadEvent>,
    current_progress: Option<nezha_core::LoadProgress>,
}

pub struct App {
    pub render_ctx: RenderContext,
    pub project: ProjectState,
    pub ui: UiState,
    midi_loader: Option<MidiLoader>,
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
            midi_loader: None,
        }
    }

    pub fn pick_midi_file(&mut self) {
        if self.midi_loader.is_some() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MIDI", &["mid", "midi"])
            .pick_file()
        {
            let path_str = path.to_string_lossy().to_string();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn({
                let path = path_str.clone();
                move || {
                    let result = nezha_core::MidiFile::load_with_progress(&path, |p| {
                        let _ = tx.send(MidiLoadEvent::Progress(p));
                    });
                    let _ = tx.send(MidiLoadEvent::Complete(result));
                }
            });
            self.midi_loader = Some(MidiLoader {
                path: path_str,
                rx,
                current_progress: None,
            });
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

    // ── Input handling ─────────────────────────────────────────────────────

    fn handle_input(&mut self, ui: &mut egui::Ui) {
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
    }

    // ── Side panels ────────────────────────────────────────────────────────

    fn render_side_panels(&mut self, ui: &mut egui::Ui) {
        let mut config_action: Option<config_panel::ConfigAction> = None;
        let dark_mode = self.ui.theme_mode.is_dark(ui.ctx());
        self.project.timeline_state.fps = self.project.fps;
        let dur = self.project.duration() as f32;

        // Left sidebar
        egui::Panel::left("sidebar")
            .exact_size(60.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                sidebar::show(ui, &mut self.ui.active_tab);
            });

        // Bottom transport
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

        // Config panel
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

        // Properties panel
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

        if let Some(action) = config_action {
            self.handle_config_action(action);
        }
    }

    // ── Central preview ────────────────────────────────────────────────────

    fn render_preview(&mut self, ui: &mut egui::Ui) {
        self.update_playback();

        let available = ui.available_size();
        let rw = self.project.render_width as f32;
        let rh = self.project.render_height as f32;
        let current_time = self.project.current_time as f32;
        let render_w = self.project.render_width;
        let render_h = self.project.render_height;

        let layers = self.collect_visible_layers(current_time);
        let default_style = self.default_style();

        // Solid-color background
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

        // Waterfall layers, bottom-to-top
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
    }

    // ── MIDI loading overlay ───────────────────────────────────────────────

    fn show_midi_loading(&mut self, ui: &mut egui::Ui) {
        if let Some(mut loader) = self.midi_loader.take() {
            let mut done = false;
            while let Ok(event) = loader.rx.try_recv() {
                match event {
                    MidiLoadEvent::Progress(p) => loader.current_progress = Some(p),
                    MidiLoadEvent::Complete(result) => {
                        match result {
                            Ok(midi) => {
                                let path = loader.path.clone();
                                let _idx = self.project.insert_midi(path, midi);
                                self.render_ctx.reset_midi_state();
                            }
                            Err(e) => {
                                self.project.last_error = Some(format!("MIDI 加载失败: {}", e));
                            }
                        }
                        done = true;
                        break;
                    }
                }
            }
            if !done {
                self.midi_loader = Some(loader);
            }
        }

        if let Some(loader) = &self.midi_loader {
            let screen_rect = ui.ctx().content_rect();
            ui.ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    "midi_loading_overlay".into(),
                ))
                .rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
                );

            egui::Window::new("正在加载 MIDI")
                .order(egui::Order::Tooltip)
                .collapsible(false)
                .resizable(false)
                .movable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    if let Some(progress) = &loader.current_progress {
                        ui.label(format!(
                            "正在解析音轨 {} / {}",
                            progress.current_track, progress.total_tracks
                        ));
                        let ratio =
                            progress.current_track as f32 / progress.total_tracks.max(1) as f32;
                        ui.add(egui::ProgressBar::new(ratio).show_percentage());
                    } else {
                        ui.label("正在读取文件...");
                        ui.add(egui::Spinner::new());
                    }
                });
        }
    }

    // ── Error toast ────────────────────────────────────────────────────────

    fn show_error_toast(&mut self, ui: &mut egui::Ui) {
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "profiling")]
        puffin::GlobalProfiler::lock().new_frame();

        self.ui.theme_mode.apply(ui.ctx());
        self.handle_input(ui);

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_side_panels(ui);

            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.render_preview(ui);
            });

            ui.ctx().request_repaint();
        });

        self.show_midi_loading(ui);
        self.show_error_toast(ui);
    }
}
