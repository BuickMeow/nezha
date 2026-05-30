use std::sync::Arc;

use ab_glyph::{Font as _, FontRef as AbFontRef};

/// 字体行高信息。
#[derive(Clone, Copy, Debug)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
}

/// A lightweight wrapper around an `ab_glyph::FontRef` that can be shared across atlases.
pub struct FontRef {
    pub(crate) inner: AbFontRef<'static>,
}

impl FontRef {
    /// Load a font from raw bytes (e.g. an `.otf` or `.ttf` file).
    pub fn from_bytes(bytes: &'static [u8]) -> Result<Arc<Self>, &'static str> {
        let font = AbFontRef::try_from_slice(bytes).map_err(|_| "failed to parse font")?;
        Ok(Arc::new(Self { inner: font }))
    }

    /// 获取指定字号的行高信息。
    pub fn line_metrics(&self, px: f32) -> Option<LineMetrics> {
        let units_per_em = self.inner.units_per_em().unwrap_or(1000.0);
        let scale = px / units_per_em;
        let ascent = self.inner.ascent_unscaled() * scale;
        let descent = self.inner.descent_unscaled() * scale;
        let line_gap = self.inner.line_gap_unscaled() * scale;
        Some(LineMetrics {
            ascent,
            descent,
            line_gap,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ab_glyph::PxScale;

    #[test]
    fn test_digit_metrics_at_16px() {
        let bytes: &'static [u8] = include_bytes!("../../../assets/MiSans-Regular.otf");
        let font = FontRef::from_bytes(bytes).unwrap();

        let px = 16.0;
        let lm = font.line_metrics(px).unwrap();
        println!(
            "ascent={:.2} descent={:.2} line_gap={:.2}",
            lm.ascent, lm.descent, lm.line_gap
        );

        let units_per_em = font.inner.units_per_em().unwrap_or(1000.0);
        let scale = PxScale::from(px);
        for c in ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'] {
            let glyph_id = font.inner.glyph_id(c);
            let glyph = glyph_id.with_scale(scale);
            if let Some(outlined) = font.inner.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let advance = font.inner.h_advance_unscaled(glyph_id) * px / units_per_em;
                println!(
                    "{}: xmin={:.3} ymin={:.3} w={:.3} h={:.3} adv={:.3}",
                    c,
                    bounds.min.x,
                    bounds.min.y,
                    bounds.width(),
                    bounds.height(),
                    advance,
                );
            }
        }
    }
}
