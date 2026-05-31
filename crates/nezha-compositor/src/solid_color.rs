use std::collections::HashMap;

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    ColorTargetState, ColorWrites, Device, FragmentState, MultisampleState,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    TextureFormat, VertexState,
};

use crate::layer::{BlendMode, LayerRenderer};
use crate::util::blend_state_for;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SolidColorUniforms {
    color: [f32; 4],
}

/// A layer that fills a rectangle with a solid color.
pub struct SolidColorLayer {
    pipelines: HashMap<BlendMode, RenderPipeline>,
    bind_group: BindGroup,
    uniform_buffer: Buffer,
}

impl SolidColorLayer {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat, color: [f64; 4]) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("solid_color_shader"),
            source: ShaderSource::Wgsl(include_str!("solid_color.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("solid_color_uniforms"),
            size: std::mem::size_of::<SolidColorUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("solid_color_bind_group_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("solid_color_bind_group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("solid_color_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let mut pipelines = HashMap::new();
        for mode in [BlendMode::Normal, BlendMode::Add, BlendMode::Multiply] {
            let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(&format!("solid_color_pipeline_{:?}", mode)),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
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
                    ..PrimitiveState::default()
                },
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });
            pipelines.insert(mode, pipeline);
        }

        let layer = Self {
            pipelines,
            bind_group,
            uniform_buffer,
        };
        layer.set_color(queue, color);
        layer
    }

    pub fn set_color(&self, queue: &Queue, color: [f64; 4]) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&SolidColorUniforms {
                color: [
                    color[0] as f32,
                    color[1] as f32,
                    color[2] as f32,
                    color[3] as f32,
                ],
            }),
        );
    }
}

impl LayerRenderer for SolidColorLayer {
    fn prepare(&mut self, _width: u32, _height: u32, _time: f64) {}

    fn render(&mut self, params: crate::layer::LayerRenderParams<'_>) {
        let mut pass = crate::util::begin_layer_pass(
            params.encoder,
            params.target,
            params.load_op,
            "solid_color_pass",
            params.rect,
            params.width,
            params.height,
        );

        let pipeline = self.pipelines.get(&params.blend_mode).unwrap_or_else(|| {
            self.pipelines
                .get(&BlendMode::Normal)
                .expect("Normal pipeline always exists")
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
