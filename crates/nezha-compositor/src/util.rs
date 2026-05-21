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

#[cfg(test)]
mod tests {
    use super::*;

    // ── compute_scissor_rect ──

    #[test]
    fn test_full_rect() {
        let (sx, sy, sw, sh) = compute_scissor_rect((0.0, 0.0, 1.0, 1.0), 1920, 1080);
        assert_eq!(sx, 0);
        assert_eq!(sy, 0);
        assert_eq!(sw, 1920);
        assert_eq!(sh, 1080);
    }

    #[test]
    fn test_partial_rect() {
        let (sx, sy, sw, sh) = compute_scissor_rect((0.25, 0.25, 0.5, 0.5), 1920, 1080);
        assert_eq!(sx, 480);
        assert_eq!(sy, 270);
        assert_eq!(sw, 960);
        assert_eq!(sh, 540);
    }

    #[test]
    fn test_clamp_overflow() {
        let (sx, sy, sw, sh) = compute_scissor_rect((-0.1, -0.1, 1.5, 1.5), 1920, 1080);
        // sx/sy clamped to 0
        assert_eq!(sx, 0);
        assert_eq!(sy, 0);
        // sw/sh clamped to not exceed width - sx
        assert_eq!(sw, 1920);
        assert_eq!(sh, 1080);
    }

    #[test]
    fn test_min_positive_size() {
        let (_sx, _sy, sw, sh) = compute_scissor_rect((0.5, 0.5, 0.0, 0.0), 100, 100);
        // sw/sh clamped to at least 1
        assert!(sw >= 1);
        assert!(sh >= 1);
    }

    #[test]
    fn test_offset_rect() {
        let (sx, sy, sw, sh) = compute_scissor_rect((0.1, 0.2, 0.3, 0.4), 800, 600);
        assert_eq!(sx, 80);
        assert_eq!(sy, 120);
        assert_eq!(sw, 240);
        assert_eq!(sh, 240);
    }

    // ── blend_state_for ──

    #[test]
    fn test_blend_normal() {
        let bs = blend_state_for(BlendMode::Normal);
        // ALPHA_BLENDING: src_alpha * src + (1 - src_alpha) * dst
        assert_eq!(
            bs.color.src_factor,
            BlendFactor::SrcAlpha,
            "normal color src_factor"
        );
        assert_eq!(
            bs.color.dst_factor,
            BlendFactor::OneMinusSrcAlpha,
            "normal color dst_factor"
        );
    }

    #[test]
    fn test_blend_add() {
        let bs = blend_state_for(BlendMode::Add);
        assert_eq!(bs.color.src_factor, BlendFactor::One);
        assert_eq!(bs.color.dst_factor, BlendFactor::One);
        assert_eq!(bs.color.operation, BlendOperation::Add);
    }

    #[test]
    fn test_blend_multiply() {
        let bs = blend_state_for(BlendMode::Multiply);
        assert_eq!(bs.color.src_factor, BlendFactor::Dst);
        assert_eq!(bs.color.dst_factor, BlendFactor::Zero);
        assert_eq!(bs.color.operation, BlendOperation::Add);
    }
}
