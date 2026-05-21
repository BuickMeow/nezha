use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use nezha_compositor::{BlendMode, LayerRenderer};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BlendComponent, BlendFactor, BlendOperation, BlendState,
    Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, CommandEncoder, Device,
    FragmentState, FrontFace, LoadOp, MultisampleState, PipelineCompilationOptions,
    PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor,
    ShaderSource, TextureFormat, TextureView, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
};

use crate::atlas::FontAtlas;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextUniforms {
    color: [f32; 4],
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// A compositor layer that renders a string of text using a GPU glyph atlas.
pub struct TextLayer<'a> {
    atlas: &'a mut FontAtlas,
    device: Device,
    queue: Queue,
    text: String,
    color: [f32; 4],
    font_size: u32,
    position: [f32; 2],
    dirty: bool,

    vertex_buffer: Buffer,
    vertex_capacity: usize,
    num_vertices: u32,
    uniform_buffer: Buffer,
    bind_group: BindGroup,
    pipelines: HashMap<BlendMode, RenderPipeline>,
}

impl<'a> TextLayer<'a> {
    pub fn new(
        atlas: &'a mut FontAtlas,
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
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
            atlas,
            device: device.clone(),
            queue: queue.clone(),
            text: String::new(),
            color: [1.0, 1.0, 1.0, 1.0],
            font_size: 24,
            position: [0.0, 0.0],
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

    fn rebuild_vertices(&mut self) {
        let mut vertices = Vec::with_capacity(self.text.len() * 6);
        let mut pen_x = self.position[0];
        let baseline_y = self.position[1] + self.font_size as f32;

        for c in self.text.chars() {
            let Some(glyph) = self
                .atlas
                .glyph(c, self.font_size, &self.device, &self.queue)
            else {
                // Skip unrenderable glyphs.
                continue;
            };

            if glyph.size[0] > 0.0 && glyph.size[1] > 0.0 {
                let x0 = pen_x + glyph.offset[0];
                let y0 = baseline_y + glyph.offset[1];
                let x1 = x0 + glyph.size[0];
                let y1 = y0 + glyph.size[1];

                let u0 = glyph.uv[0];
                let v0 = glyph.uv[1];
                let u1 = u0 + glyph.uv[2];
                let v1 = v0 + glyph.uv[3];

                // Two triangles per glyph.
                vertices.push(TextVertex {
                    position: [x0, y0],
                    uv: [u0, v0],
                });
                vertices.push(TextVertex {
                    position: [x1, y0],
                    uv: [u1, v0],
                });
                vertices.push(TextVertex {
                    position: [x0, y1],
                    uv: [u0, v1],
                });

                vertices.push(TextVertex {
                    position: [x0, y1],
                    uv: [u0, v1],
                });
                vertices.push(TextVertex {
                    position: [x1, y0],
                    uv: [u1, v0],
                });
                vertices.push(TextVertex {
                    position: [x1, y1],
                    uv: [u1, v1],
                });
            }

            pen_x += glyph.advance;
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

impl<'a> LayerRenderer for TextLayer<'a> {
    fn prepare(&mut self, width: u32, height: u32, _time: f64) {
        if self.dirty {
            self.rebuild_vertices();
            self.dirty = false;
        }

        let uniforms = TextUniforms {
            color: self.color,
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    fn render(
        &mut self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        width: u32,
        height: u32,
        _time: f64,
        load_op: LoadOp<wgpu::Color>,
        blend_mode: BlendMode,
        rect: (f32, f32, f32, f32),
    ) {
        if self.num_vertices == 0 {
            return;
        }

        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("text_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            multiview_mask: None,
            timestamp_writes: None,
        });

        let sx = (rect.0 * width as f32).clamp(0.0, width as f32) as u32;
        let sy = (rect.1 * height as f32).clamp(0.0, height as f32) as u32;
        let sw = (rect.2 * width as f32).clamp(1.0, (width - sx) as f32) as u32;
        let sh = (rect.3 * height as f32).clamp(1.0, (height - sy) as f32) as u32;
        pass.set_scissor_rect(sx, sy, sw, sh);

        let pipeline = self
            .pipelines
            .get(&blend_mode)
            .unwrap_or_else(|| self.pipelines.get(&BlendMode::Normal).unwrap());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.num_vertices, 0..1);
    }
}

fn blend_state_for(mode: BlendMode) -> BlendState {
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
