use std::sync::Arc;

/// A lightweight wrapper around a `fontdue::Font` that can be shared across atlases.
pub struct FontRef {
    pub(crate) inner: fontdue::Font,
}

impl FontRef {
    /// Load a font from raw bytes (e.g. an `.otf` or `.ttf` file).
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, &'static str> {
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map_err(|_| "failed to parse font")?;
        Ok(Arc::new(Self { inner: font }))
    }
}
