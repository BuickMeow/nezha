use crate::layer::{Layer, LayerRenderer};

/// Orchestrates rendering of multiple layers onto a single output target.
#[derive(Default)]
pub struct Compositor;

impl Compositor {
    pub fn new() -> Self {
        Self
    }

    /// Render a single layer with the specified load operation.
    ///
    /// This is the primary API for stage 1, where layers are rendered
    /// sequentially due to mutable-borrow constraints on shared renderers.
    pub fn render_layer(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        renderer: &mut dyn LayerRenderer,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f64,
        load_op: wgpu::LoadOp<wgpu::Color>,
    ) {
        renderer.prepare(width, height, time);
        renderer.render(encoder, target, width, height, time, load_op);
    }

    /// Render multiple layers in batch.
    ///
    /// This is the future-facing API for full layer compositing.
    /// Currently limited by lifetime constraints when multiple layers need
    /// mutable access to shared renderers (e.g. multiple Waterfall clips).
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        layers: &mut [Layer<'_>],
        final_target: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f64,
        clear_color: Option<wgpu::Color>,
    ) {
        for (i, layer) in layers.iter_mut().enumerate() {
            let is_first = i == 0;
            let load_op = if is_first && clear_color.is_some() {
                wgpu::LoadOp::Clear(clear_color.unwrap())
            } else {
                wgpu::LoadOp::Load
            };
            self.render_layer(
                encoder,
                layer.renderer,
                final_target,
                width,
                height,
                time,
                load_op,
            );
        }
    }
}
