use eframe::egui;

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
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 加载自定义字体
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
            width: 800,
            height: 600,
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
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 非UI更新逻辑
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 1. 最左侧导航栏
        egui::Panel::left("sidebar")
            .exact_size(60.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                sidebar::show(ui, &mut self.active_tab);
            });

        // 2. 配置面板
        egui::Panel::left("config_panel")
            .exact_size(260.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                config_panel::show(ui, self.active_tab);
            });

        // 3. 底部走带
        egui::Panel::bottom("transport")
            .exact_size(60.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                transport::show(ui, &mut self.is_playing, &mut self.current_time, self.duration);
            });

        // 4. 主钢琴窗口
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // 更新播放时间
            if self.is_playing {
                self.current_time += ui.input(|i| i.unstable_dt);
                if self.current_time > self.duration {
                    self.current_time = 0.0;
                }
            }

            // 渲染到 texture
            self.renderer.render(
                &self.preview_view, 800, 600, self.current_time);

            let available = ui.available_size();
            piano_view::show(ui, self.preview_texture_id, available);
        });

        ui.ctx().request_repaint();
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
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
