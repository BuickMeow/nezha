use crate::layer::{LayerRenderParams, LayerRenderer};

/// Render a single layer with the specified load operation.
///
/// Calls `prepare` then `render` on the given renderer.
pub fn render_layer(
    encoder: &mut wgpu::CommandEncoder,
    renderer: &mut dyn LayerRenderer,
    target: &wgpu::TextureView,
    width: u32,
    height: u32,
    time: f64,
    load_op: wgpu::LoadOp<wgpu::Color>,
    blend_mode: crate::BlendMode,
    rect: (f32, f32, f32, f32),
) {
    renderer.prepare(width, height, time);
    renderer.render(LayerRenderParams {
        encoder,
        target,
        width,
        height,
        time,
        load_op,
        blend_mode,
        rect,
    });
}
