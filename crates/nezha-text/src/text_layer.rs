use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use nezha_compositor::{BlendMode, LayerRenderer, begin_layer_pass, blend_state_for};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, Device, FragmentState, FrontFace, MultisampleState, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor,
    ShaderSource, TextureFormat, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
    VertexStepMode,
};

use crate::atlas::FontAtlas;

/// 文本对齐方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextUniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// A compositor layer that renders a string of text using a GPU glyph atlas.
#[allow(dead_code)] // atlas_texture_view, atlas_sampler, format: stored to keep GPU resources alive
pub struct TextLayer {
    atlas_texture_view: wgpu::TextureView,
    atlas_sampler: wgpu::Sampler,
    format: TextureFormat,
    device: Device,
    queue: Queue,
    text: String,
    color: [f32; 4],
    font_size: u32,
    position: [f32; 2],
    alignment: Alignment,
    // ── 描边 ──
    outline_width: f32,
    outline_color: [f32; 4],
    // ── 粗体 ──
    bold: bool,
    bold_offset: f32,
    // ── 斜体 ──
    italic: bool,
    italic_slant: f32,
    // ── 字间距 ──
    letter_spacing: f32,
    // ── 最小字宽 ──
    min_advance: f32,

    dirty: bool,

    vertex_buffer: Buffer,
    vertex_capacity: usize,
    num_vertices: u32,
    uniform_buffer: Buffer,
    bind_group: BindGroup,
    pipelines: HashMap<BlendMode, RenderPipeline>,
}

impl TextLayer {
    pub fn new(
        atlas: &FontAtlas,
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
    ) -> Self {
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("text_vertex_buffer"),
            size: std::mem::size_of::<TextVertex>() as u64 * 6,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("text_uniform_buffer"),
            size: std::mem::size_of::<TextUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("text_bind_group_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("text_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(atlas.texture_view()),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(atlas.sampler()),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: ShaderSource::Wgsl(include_str!("text.wgsl").into()),
        });

        let mut pipelines = HashMap::new();
        for mode in [BlendMode::Normal, BlendMode::Add, BlendMode::Multiply] {
            let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(&format!("text_pipeline_{:?}", mode)),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[VertexBufferLayout {
                        array_stride: std::mem::size_of::<TextVertex>() as u64,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 1,
                            },
                            VertexAttribute {
                                format: VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 2,
                            },
                        ],
                    }],
                    compilation_options: PipelineCompilationOptions::default(),
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(ColorTargetState {
                        format,
                        blend: Some(blend_state_for(mode)),
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    front_face: FrontFace::Ccw,
                    ..PrimitiveState::default()
                },
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });
            pipelines.insert(mode, pipeline);
        }

        Self {
            atlas_texture_view: atlas.texture_view().clone(),
            atlas_sampler: atlas.sampler().clone(),
            format,
            device: device.clone(),
            queue: queue.clone(),
            text: String::new(),
            color: [1.0, 1.0, 1.0, 1.0],
            font_size: 24,
            position: [0.0, 0.0],
            alignment: Alignment::default(),
            outline_width: 0.0,
            outline_color: [0.0, 0.0, 0.0, 1.0],
            bold: false,
            bold_offset: 1.0,
            italic: false,
            italic_slant: 0.0,
            letter_spacing: 0.0,
            min_advance: 0.0,
            dirty: true,
            vertex_buffer,
            vertex_capacity: 1,
            num_vertices: 0,
            uniform_buffer,
            bind_group,
            pipelines,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            self.dirty = true;
        }
    }

    pub fn set_color(&mut self, color: [f32; 4]) {
        self.color = color;
    }

    pub fn set_font_size(&mut self, size: u32) {
        if self.font_size != size {
            self.font_size = size;
            self.dirty = true;
        }
    }

    pub fn set_position(&mut self, pos: [f32; 2]) {
        if self.position != pos {
            self.position = pos;
            self.dirty = true;
        }
    }

    pub fn set_alignment(&mut self, align: Alignment) {
        if self.alignment != align {
            self.alignment = align;
            self.dirty = true;
        }
    }

    pub fn set_outline(&mut self, width: f32, color: [f32; 4]) {
        self.outline_width = width;
        self.outline_color = color;
    }

    pub fn set_bold(&mut self, bold: bool) {
        if self.bold != bold {
            self.bold = bold;
            self.dirty = true;
        }
    }

    pub fn set_bold_offset(&mut self, offset: f32) {
        if self.bold_offset != offset {
            self.bold_offset = offset;
            self.dirty = true;
        }
    }

    pub fn set_italic(&mut self, italic: bool) {
        if self.italic != italic {
            self.italic = italic;
            self.dirty = true;
        }
    }

    pub fn set_italic_slant(&mut self, slant: f32) {
        if self.italic_slant != slant {
            self.italic_slant = slant;
            self.dirty = true;
        }
    }

    pub fn set_letter_spacing(&mut self, spacing: f32) {
        if self.letter_spacing != spacing {
            self.letter_spacing = spacing;
            self.dirty = true;
        }
    }

    pub fn set_min_advance(&mut self, min: f32) {
        if self.min_advance != min {
            self.min_advance = min;
            self.dirty = true;
        }
    }

    /// 应用斜切变换到坐标。
    fn apply_slant(&self, x: f32, y: f32, baseline_y: f32) -> [f32; 2] {
        if !self.italic || self.italic_slant == 0.0 {
            return [x, y];
        }
        // 以 baseline 为参考：baseline 上方的点向右偏移更多
        let dy = baseline_y - y;
        [x + dy * self.italic_slant, y]
    }

    /// 为单个 glyph 的单个位置生成 6 个顶点（2 个三角形）。
    fn push_glyph(
        &self,
        vertices: &mut Vec<TextVertex>,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
        baseline_y: f32,
    ) {
        let p00 = self.apply_slant(x0, y0, baseline_y);
        let p10 = self.apply_slant(x1, y0, baseline_y);
        let p01 = self.apply_slant(x0, y1, baseline_y);
        let p11 = self.apply_slant(x1, y1, baseline_y);

        vertices.push(TextVertex {
            position: p00,
            uv: [u0, v0],
            color,
        });
        vertices.push(TextVertex {
            position: p10,
            uv: [u1, v0],
            color,
        });
        vertices.push(TextVertex {
            position: p01,
            uv: [u0, v1],
            color,
        });
        vertices.push(TextVertex {
            position: p01,
            uv: [u0, v1],
            color,
        });
        vertices.push(TextVertex {
            position: p10,
            uv: [u1, v0],
            color,
        });
        vertices.push(TextVertex {
            position: p11,
            uv: [u1, v1],
            color,
        });
    }

    fn rebuild_vertices(&mut self, atlas: &mut FontAtlas) {
        let mut vertices = Vec::with_capacity(self.text.len() * 6 * 10);

        // 获取字体真实 metrics，用 ascent 作为 baseline 到顶部的距离。
        let ascent = atlas
            .line_metrics(self.font_size)
            .map(|m| m.ascent)
            .unwrap_or(self.font_size as f32);

        // First pass: measure each line and build glyph positions.
        let lines: Vec<&str> = self.text.lines().collect();
        let mut line_measurements: Vec<(f32, Vec<(char, f32, crate::atlas::GlyphInfo)>)> =
            Vec::new();

        for line in &lines {
            let mut pen_x = 0.0f32;
            let mut glyphs: Vec<(char, f32, crate::atlas::GlyphInfo)> = Vec::new();
            for c in line.chars() {
                let Some(glyph) = atlas
                    .glyph(c, self.font_size, &self.device, &self.queue)
                else {
                    continue;
                };
                let info = *glyph;
                glyphs.push((c, pen_x, info));
                let effective_advance = info.advance.max(self.min_advance);
                pen_x += effective_advance + self.letter_spacing;
            }
            line_measurements.push((pen_x, glyphs));
        }

        let line_height = self.font_size as f32 * 1.2;
        let total_height = lines.len() as f32 * line_height;

        // 描边/粗体偏移方向（8 方向）
        let offset_dirs: &[(f32, f32)] = &[
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-1.0, -1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ];

        for (line_idx, (_line_width, glyphs)) in line_measurements.iter().enumerate() {
            let offset_x = match self.alignment {
                Alignment::TopLeft | Alignment::BottomLeft => 0.0,
                Alignment::TopRight | Alignment::BottomRight => -_line_width,
            };
            let offset_y = match self.alignment {
                Alignment::TopLeft | Alignment::TopRight => line_idx as f32 * line_height,
                Alignment::BottomLeft | Alignment::BottomRight => {
                    -(total_height - line_idx as f32 * line_height)
                }
            };

            let baseline_x = self.position[0] + offset_x;
            let baseline_y = self.position[1] + ascent + offset_y;

            for (_c, pen_x, glyph) in glyphs.iter() {
                if glyph.size[0] > 0.0 && glyph.size[1] > 0.0 {
                    let base_x0 = baseline_x + pen_x + glyph.offset[0];
                    let base_y0 = baseline_y + glyph.offset[1];
                    let base_x1 = base_x0 + glyph.size[0];
                    let base_y1 = base_y0 + glyph.size[1];

                    let u0 = glyph.uv[0];
                    let v0 = glyph.uv[1];
                    let u1 = u0 + glyph.uv[2];
                    let v1 = v0 + glyph.uv[3];

                    // ── 描边（最底层，8 方向偏移）──
                    if self.outline_width > 0.0 {
                        for (dx, dy) in offset_dirs {
                            let ox = dx * self.outline_width;
                            let oy = dy * self.outline_width;
                            self.push_glyph(
                                &mut vertices,
                                base_x0 + ox,
                                base_y0 + oy,
                                base_x1 + ox,
                                base_y1 + oy,
                                u0,
                                v0,
                                u1,
                                v1,
                                self.outline_color,
                                baseline_y,
                            );
                        }
                    }

                    // ── 粗体（叠加偏移，用正文颜色）──
                    if self.bold && self.bold_offset > 0.0 {
                        for (dx, dy) in offset_dirs {
                            let ox = dx * self.bold_offset;
                            let oy = dy * self.bold_offset;
                            self.push_glyph(
                                &mut vertices,
                                base_x0 + ox,
                                base_y0 + oy,
                                base_x1 + ox,
                                base_y1 + oy,
                                u0,
                                v0,
                                u1,
                                v1,
                                self.color,
                                baseline_y,
                            );
                        }
                    }

                    // ── 正文（最上层）──
                    self.push_glyph(
                        &mut vertices,
                        base_x0,
                        base_y0,
                        base_x1,
                        base_y1,
                        u0,
                        v0,
                        u1,
                        v1,
                        self.color,
                        baseline_y,
                    );
                }
            }
        }

        self.num_vertices = vertices.len() as u32;

        if vertices.is_empty() {
            return;
        }

        let needed = vertices.len();
        if needed > self.vertex_capacity {
            self.vertex_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("text_vertex_buffer"),
                size: (std::mem::size_of::<TextVertex>() * needed) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = needed;
        }

        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }
}

/// Rebuild vertices if the text layer is dirty (text, font size, position, etc. changed).
///
/// This is a standalone function because `rebuild_vertices` needs `&mut FontAtlas`
/// (for lazy glyph rasterization), while `LayerRenderer::prepare` only has `&mut self`.
/// Call this before `render_layer` to ensure vertices are up-to-date.
pub fn prepare_text(layer: &mut TextLayer, atlas: &mut FontAtlas) {
    if layer.dirty {
        layer.rebuild_vertices(atlas);
        layer.dirty = false;
    }
}

impl LayerRenderer for TextLayer {
    fn prepare(&mut self, width: u32, height: u32, _time: f64) {
        let uniforms = TextUniforms {
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn render(&mut self, params: nezha_compositor::LayerRenderParams<'_>) {
        if self.num_vertices == 0 {
            return;
        }

        let mut pass = begin_layer_pass(
            params.encoder,
            params.target,
            params.load_op,
            "text_pass",
            params.rect,
            params.width,
            params.height,
        );

        let pipeline = self
            .pipelines
            .get(&params.blend_mode)
            .unwrap_or_else(|| self.pipelines.get(&BlendMode::Normal).unwrap());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.num_vertices, 0..1);
    }
}
