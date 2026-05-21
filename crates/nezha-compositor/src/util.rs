use crate::layer::BlendMode;
use wgpu::{BlendComponent, BlendFactor, BlendOperation, BlendState};

/// Compute a clamped scissor rectangle from a normalized rect (0..1) and pixel dimensions.
pub fn compute_scissor_rect(
    rect: (f32, f32, f32, f32),
    width: u32,
    height: u32,
) -> (u32, u32, u32, u32) {
    let sx = (rect.0 * width as f32).clamp(0.0, width as f32) as u32;
    let sy = (rect.1 * height as f32).clamp(0.0, height as f32) as u32;
    let sw = (rect.2 * width as f32).clamp(1.0, (width - sx) as f32) as u32;
    let sh = (rect.3 * height as f32).clamp(1.0, (height - sy) as f32) as u32;
    (sx, sy, sw, sh)
}

/// Convert a [`BlendMode`] into a wgpu [`BlendState`].
pub fn blend_state_for(mode: BlendMode) -> BlendState {
    match mode {
        BlendMode::Normal => BlendState::ALPHA_BLENDING,
        BlendMode::Add => BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        },
        BlendMode::Multiply => BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::Dst,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        },
    }
}
