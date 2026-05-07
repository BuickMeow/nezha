use eframe::egui;
use nezha_core::MidiFile;
use std::sync::Arc;

mod sidebar;
mod config_panel;
mod piano_view;
mod transport;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn is_dark(&self, ctx: &egui::Context) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => ctx.global_style().visuals.dark_mode,
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        match self {
            ThemeMode::Dark => ctx.set_theme(egui::ThemePreference::Dark),
            ThemeMode::Light => ctx.set_theme(egui::ThemePreference::Light),
            ThemeMode::System => ctx.set_theme(egui::ThemePreference::System),
        }
    }
}

pub struct App {
    wgpu_state: Arc<eframe::egui_wgpu::RenderState>,
    renderer: nezha_renderer::Renderer,
    _preview_texture: wgpu::Texture,
    preview_view: wgpu::TextureView,
    preview_texture_id: egui::TextureId,
    active_tab: sidebar::SidebarTab,
    is_playing: bool,
    current_time: f32,
    duration: f32,
    midi_file: Option<MidiFile>,
    midi_path: Option<String>,
    pending_midi_load: Option<String>,
    render_width: u32,
    render_height: u32,
    fps: u32,
    needs_resize: bool,
    timeline_state: transport::TimelineState,
    export_format: String,
    encoder: String,
    export_path: Option<String>,
    bg_color: [u8; 3],
    note_color: [u8; 3],
    theme_mode: ThemeMode,
    zoom: f32,
    pan_offset: egui::Vec2,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
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

        let wgpu_state = cc
            .wgpu_render_state
            .clone()
            .expect("wgpu backend required");
        let device = &wgpu_state.device;
        let queue = &wgpu_state.queue;

        let format = wgpu_state.target_format;
        let renderer = nezha_renderer::Renderer::new(device.clone(), queue.clone(), format);

        let (preview_texture, preview_view, preview_texture_id) =
            Self::create_preview(device, &mut wgpu_state.renderer.write(), format, 1920, 1080);

        let mut timeline_state = transport::TimelineState::default();
        timeline_state.fps = 60;

        Self {
            wgpu_state: wgpu_state.into(),
            renderer,
            _preview_texture: preview_texture,
            preview_view,
            preview_texture_id,
            active_tab: sidebar::SidebarTab::Midi,
            is_playing: false,
            current_time: 0.0,
            duration: 120.0,
            midi_file: None,
            midi_path: None,
            pending_midi_load: None,
            render_width: 1920,
            render_height: 1080,
            fps: 60,
            needs_resize: false,
            timeline_state,
            export_format: "MP4".to_string(),
            encoder: "H.264".to_string(),
            export_path: None,
            bg_color: [0, 0, 0],
            note_color: [100, 150, 255],
            theme_mode,
            zoom: 1.0,
            pan_offset: egui::Vec2::ZERO,
        }
    }

    fn create_preview(
        device: &wgpu::Device,
        egui_renderer: &mut eframe::egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView, egui::TextureId) {
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let preview_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("preview_texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let preview_view = preview_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let preview_texture_id = egui_renderer.register_native_texture(
            device,
            &preview_view,
            wgpu::FilterMode::Linear,
        );
        (preview_texture, preview_view, preview_texture_id)
    }

    fn load_midi(&mut self, path: String) {
        match MidiFile::load(&path) {
            Ok(midi) => {
                self.duration = midi.duration;
                self.midi_path = Some(path);
                self.timeline_state.update_duration(self.duration);
                self.midi_file = Some(midi);
                self.current_time = 0.0;
            }
            Err(e) => {
                eprintln!("Failed to load MIDI: {}", e);
            }
        }
    }

    fn check_file_dialog(&mut self) {
        if self.pending_midi_load.is_none() && self.active_tab == sidebar::SidebarTab::Midi {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("MIDI", &["mid", "midi"])
                .pick_file()
            {
                self.pending_midi_load = Some(path.to_string_lossy().to_string());
            }
        }
    }

    fn process_pending(&mut self) {
        if let Some(path) = self.pending_midi_load.take() {
            self.load_midi(path);
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_pending();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 应用主题变更
        self.theme_mode.apply(ui.ctx());

        // 空格键播放/停止
        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            self.is_playing = !self.is_playing;
        }

        let midi_path_clone = self.midi_path.clone();
        let mut should_open_dialog = false;

        // 检测分辨率变更并重建 texture
        if self.needs_resize {
            self.needs_resize = false;
            let format = self.wgpu_state.target_format;
            let device = &self.wgpu_state.device;
            let mut egui_renderer = self.wgpu_state.renderer.write();
            egui_renderer.free_texture(&self.preview_texture_id);
            let (tex, view, id) = Self::create_preview(
                device, &mut egui_renderer, format, self.render_width, self.render_height,
            );
            self._preview_texture = tex;
            self.preview_view = view;
            self.preview_texture_id = id;
        }

        egui::Panel::left("sidebar")
            .exact_size(60.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                sidebar::show(ui, &mut self.active_tab);
            });

        egui::Panel::left("config_panel")
            .exact_size(260.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                config_panel::show(
                    ui,
                    self.active_tab,
                    &midi_path_clone,
                    &mut || {
                        should_open_dialog = true;
                    },
                    &mut self.render_width,
                    &mut self.render_height,
                    &mut self.fps,
                    &mut self.needs_resize,
                    &mut self.export_format,
                    &mut self.encoder,
                    &mut self.export_path,
                    &mut self.bg_color,
                    &mut self.note_color,
                    &mut self.theme_mode,
                );
            });

        if should_open_dialog {
            self.check_file_dialog();
        }

        let dark_mode = self.theme_mode.is_dark(ui.ctx());
        self.timeline_state.fps = self.fps;

        egui::Panel::bottom("transport")
            .exact_size(200.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                transport::show(
                    ui,
                    &mut self.is_playing,
                    &mut self.current_time,
                    self.duration,
                    &mut self.timeline_state,
                    dark_mode,
                );
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.is_playing {
                // 固定帧率步进，消除 unstable_dt 波动导致的抖动
                self.current_time += 1.0 / self.fps as f32;
                if self.current_time > self.duration {
                    self.current_time = 0.0;
                    self.is_playing = false;
                }
            }

            let available = ui.available_size();
            let rw = self.render_width as f32;
            let rh = self.render_height as f32;

            // 渲染前帧对齐，确保时间精确到帧边界
            let render_time = (self.current_time * self.fps as f32).round() / self.fps as f32;

            self.renderer.render(
                &self.preview_view,
                self.render_width,
                self.render_height,
                render_time,
                self.midi_file.as_ref(),
            );

            let aspect = rw / rh;
            piano_view::show(
                ui,
                self.preview_texture_id,
                available,
                aspect,
                &mut self.zoom,
                &mut self.pan_offset,
            );
        });

        ui.ctx().request_repaint();
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Nezha MIDI Renderer",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .unwrap();
}
