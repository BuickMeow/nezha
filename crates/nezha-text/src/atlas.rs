use std::collections::HashMap;
use std::sync::Arc;

use wgpu::{
    AddressMode, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType,
    Device, FilterMode, SamplerBindingType, SamplerDescriptor, ShaderStages, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor,
};

use crate::font::FontRef;

/// Information about a glyph that has been packed into the atlas.
#[derive(Clone, Copy, Debug)]
pub struct GlyphInfo {
    /// UV rectangle in the atlas texture (u, v, w, h) — all normalized 0..1.
    pub uv: [f32; 4],
    /// Glyph size in pixels (width, height).
    pub size: [f32; 2],
    /// Offset from the pen position to the glyph's top-left corner (x, y).
    pub offset: [f32; 2],
    /// Horizontal advance in pixels.
    pub advance: f32,
}

/// A GPU glyph atlas that rasterizes glyphs on-demand using `fontdue`.
pub struct FontAtlas {
    font: Arc<FontRef>,
    texture: Texture,
    texture_view: TextureView,
    sampler: wgpu::Sampler,
    bind_group_layout: BindGroupLayout,

    glyphs: HashMap<(char, u32), GlyphInfo>,
    pack_x: u32,
    pack_y: u32,
    pack_row_height: u32,
    size: u32,
}

impl FontAtlas {
    pub const FORMAT: TextureFormat = TextureFormat::R8Unorm;
    pub const PADDING: u32 = 2;

    pub fn new(device: &Device, queue: &wgpu::Queue, font: Arc<FontRef>) -> Self {
        let size = 2048;
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: Self::FORMAT,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("glyph_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("glyph_atlas_bind_group_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Initialize texture to transparent black.
        let zero = vec![0u8; (size * size) as usize];
        queue.write_texture(
            texture.as_image_copy(),
            &zero,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        Self {
            font,
            texture,
            texture_view,
            sampler,
            bind_group_layout,
            glyphs: HashMap::new(),
            pack_x: 0,
            pack_y: 0,
            pack_row_height: 0,
            size,
        }
    }

    pub fn texture_view(&self) -> &TextureView {
        &self.texture_view
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }

    /// Look up (and rasterize if needed) a glyph.
    pub fn glyph(
        &mut self,
        c: char,
        px: u32,
        _device: &Device,
        queue: &wgpu::Queue,
    ) -> Option<&GlyphInfo> {
        let key = (c, px);
        if self.glyphs.contains_key(&key) {
            return self.glyphs.get(&key);
        }

        let (metrics, bitmap) = self.font.inner.rasterize(c, px as f32);
        if metrics.width == 0 || metrics.height == 0 {
            // Zero-sized glyph (e.g. space). Store with empty UV.
            let info = GlyphInfo {
                uv: [0.0; 4],
                size: [0.0, 0.0],
                offset: [0.0, 0.0],
                advance: metrics.advance_width,
            };
            self.glyphs.insert(key, info);
            return self.glyphs.get(&key);
        }

        let gw = metrics.width as u32;
        let gh = metrics.height as u32;

        // Try to pack into atlas.
        if self.pack_x + gw + Self::PADDING > self.size {
            // New row.
            self.pack_x = 0;
            self.pack_y += self.pack_row_height + Self::PADDING;
            self.pack_row_height = 0;
        }

        if self.pack_y + gh + Self::PADDING > self.size {
            // Atlas overflow — for MVP just ignore this glyph.
            return None;
        }

        let x = self.pack_x;
        let y = self.pack_y;
        self.pack_row_height = self.pack_row_height.max(gh);
        self.pack_x += gw + Self::PADDING;

        // Upload bitmap to atlas.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(gw),
                rows_per_image: Some(gh),
            },
            wgpu::Extent3d {
                width: gw,
                height: gh,
                depth_or_array_layers: 1,
            },
        );

        let inv = 1.0 / self.size as f32;
        let info = GlyphInfo {
            uv: [
                x as f32 * inv,
                y as f32 * inv,
                gw as f32 * inv,
                gh as f32 * inv,
            ],
            size: [gw as f32, gh as f32],
            offset: [
                metrics.bounds.xmin,
                -(metrics.bounds.ymin + metrics.bounds.height),
            ],
            advance: metrics.advance_width,
        };
        self.glyphs.insert(key, info);
        self.glyphs.get(&key)
    }
}
