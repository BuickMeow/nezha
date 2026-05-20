use eframe::egui;

pub(super) struct PreviewTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    texture_id: egui::TextureId,
    width: u32,
    height: u32,
}

impl PreviewTarget {
    pub(super) fn new(
        device: &wgpu::Device,
        egui_renderer: &mut eframe::egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("preview_texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let texture_id =
            egui_renderer.register_native_texture(device, &view, wgpu::FilterMode::Linear);

        Self {
            _texture: texture,
            view,
            texture_id,
            width,
            height,
        }
    }

    pub(super) fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        egui_renderer: &mut eframe::egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        if self.width == width && self.height == height {
            return;
        }
        egui_renderer.free_texture(&self.texture_id);
        *self = Self::new(device, egui_renderer, format, width, height);
    }

    pub(super) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub(super) fn texture_id(&self) -> egui::TextureId {
        self.texture_id
    }

    pub(super) fn texture(&self) -> &wgpu::Texture {
        &self._texture
    }

    pub(super) fn width(&self) -> u32 {
        self.width
    }

    pub(super) fn height(&self) -> u32 {
        self.height
    }
}
