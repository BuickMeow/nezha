use eframe::egui;
use nezha_core::MidiFile;
use nezha_renderer::NoteInstance;

mod sidebar;
mod config_panel;
mod piano_view;
mod transport;

pub struct App {
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
    /// 每 key 的扫描指针，指向第一个可能可见的音符
    scan_indices: [usize; 128],
    /// 上一帧的时间，用于检测时间回退
    last_time: f32,
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

        let wgpu_render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required");
        let device = wgpu_render_state.device.clone();
        let queue = wgpu_render_state.queue.clone();

        let format = wgpu_render_state.target_format;
        let renderer = nezha_renderer::Renderer::new(device.clone(), queue.clone(), format);

        let texture_size = wgpu::Extent3d {
            width: 1280,
            height: 720,
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

        let mut renderer_lock = wgpu_render_state.renderer.write();
        let preview_texture_id = renderer_lock.register_native_texture(
            &device,
            &preview_view,
            wgpu::FilterMode::Linear,
        );

        Self {
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
            scan_indices: [0; 128],
            last_time: -1.0,
        }
    }

    fn load_midi(&mut self, path: String) {
        match MidiFile::load(&path) {
            Ok(midi) => {
                self.duration = midi.duration;
                self.midi_path = Some(path);
                self.midi_file = Some(midi);
                self.current_time = 0.0;
                self.scan_indices = [0; 128];
                self.last_time = -1.0;
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

    fn build_instances(
        &mut self,
        width: f32,
        height: f32,
        time: f32,
    ) -> Vec<NoteInstance> {
        let midi = match &self.midi_file {
            Some(m) => m,
            None => return Vec::new(),
        };

        let pps = 200.0f32;
        let key_count = 128u8;
        let key_width = width / key_count as f32;

        let visible_future = height / pps + 1.0;
        let visible_past = 1.0f32;
        let time_top = time + visible_future;
        let time_bottom = time - visible_past;

        // 时间回退时重置扫描指针
        if time < self.last_time {
            self.scan_indices = [0; 128];
        }
        self.last_time = time;

        let estimated = ((width * height) as usize / 4).clamp(50_000, 2_000_000);
        let mut instances = Vec::with_capacity(estimated);

        for key in 0..128u8 {
            let notes = &midi.key_notes[key as usize];
            if notes.is_empty() {
                continue;
            }

            let mut scan = self.scan_indices[key as usize];

            // 推进扫描指针：跳过已经彻底离开屏幕底部的音符
            // 音符按 start 排序，但 end 不一定有序，所以只能线性推进
            while scan < notes.len() && notes[scan].end < time_bottom {
                scan += 1;
            }
            self.scan_indices[key as usize] = scan;

            let x = key as f32 * key_width;
            let w = key_width;

            for i in scan..notes.len() {
                let note = &notes[i];
                if note.start > time_top {
                    break;
                }

                let start_y = height - (note.start - time) * pps;
                let end_y = height - (note.end - time) * pps;
                let y = end_y;
                let h = (start_y - end_y).max(1.0);

                let hue = (key as f32 / 128.0) * 360.0;
                let (r, g, b) = hsv_to_rgb(hue, 0.8, 1.0);

                instances.push(NoteInstance {
                    x,
                    y,
                    w,
                    h,
                    r,
                    g,
                    b,
                    a: 0.9,
                });
            }
        }

        instances
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_pending();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let midi_path_clone = self.midi_path.clone();
        let mut should_open_dialog = false;

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
                );
            });

        if should_open_dialog {
            self.check_file_dialog();
        }

        egui::Panel::bottom("transport")
            .exact_size(60.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                transport::show(ui, &mut self.is_playing, &mut self.current_time, self.duration);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.is_playing {
                self.current_time += ui.input(|i| i.unstable_dt);
                if self.current_time > self.duration {
                    self.current_time = 0.0;
                    self.is_playing = false;
                }
            }

            let available = ui.available_size();
            let width = available.x;
            let height = available.y;

            let instances = self.build_instances(width, height, self.current_time);
            self.renderer.render(
                &self.preview_view,
                1280,
                720,
                self.current_time,
                &instances,
            );

            piano_view::show(ui, self.preview_texture_id, available);
        });

        ui.ctx().request_repaint();
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (r + m, g + m, b + m)
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
