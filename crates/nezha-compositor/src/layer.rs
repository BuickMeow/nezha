#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Normal,
    Add,
    Multiply,
}

pub struct Layer<'a> {
    pub renderer: &'a mut dyn LayerRenderer,
    pub blend_mode: BlendMode,
    pub opacity: f32,
    /// Normalized rectangle (x, y, width, height) in 0..1 range.
    /// (0,0) is top-left, (1,1) is bottom-right.
    pub rect: (f32, f32, f32, f32),
}

impl<'a> Layer<'a> {
    pub fn new(renderer: &'a mut dyn LayerRenderer) -> Self {
        Self {
            renderer,
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            rect: (0.0, 0.0, 1.0, 1.0),
        }
    }

    pub fn with_blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn with_rect(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        self.rect = (x, y, w, h);
        self
    }
}

/// Trait for renderers that can draw a single compositor layer.
pub trait LayerRenderer {
    /// Prepare resources before rendering (CPU work, buffer uploads, etc.).
    fn prepare(&mut self, width: u32, height: u32, time: f64);

    /// Draw this layer into the given target.
    ///
    /// The `load_op` is controlled by the [`Compositor`] to handle
    /// clearing vs. compositing onto existing pixels.
    ///
    /// `blend_mode` and `rect` are hints the renderer may use to configure
    /// its pipeline and scissor region.
    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f64,
        load_op: wgpu::LoadOp<wgpu::Color>,
        blend_mode: BlendMode,
        rect: (f32, f32, f32, f32),
    );
}
