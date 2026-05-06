use eframe::egui;

struct App {
    renderer: nezha_renderer::Renderer,
    _preview_texture: wgpu::Texture,
    preview_view: wgpu::TextureView,
    preview_texture_id: egui::TextureId,
    start_time: std::time::Instant,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
            start_time: std::time::Instant::now(),
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 非UI更新逻辑可以放在这里
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        self.renderer.render(&self.preview_view, 800, 600, elapsed);

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.image(egui::ImageSource::Texture(egui::load::SizedTexture::new(
                self.preview_texture_id,
                [800.0, 600.0],
            )));
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
