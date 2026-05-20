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
}

impl<'a> Layer<'a> {
    pub fn new(renderer: &'a mut dyn LayerRenderer) -> Self {
        Self {
            renderer,
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
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
}

/// Trait for renderers that can draw a single compositor layer.
pub trait LayerRenderer {
    /// Prepare resources before rendering (CPU work, buffer uploads, etc.).
    fn prepare(&mut self, width: u32, height: u32, time: f64);

    /// Draw this layer into the given target.
    ///
    /// The `load_op` is controlled by the [`Compositor`] to handle
    /// clearing vs. compositing onto existing pixels.
    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f64,
        load_op: wgpu::LoadOp<wgpu::Color>,
    );
}
