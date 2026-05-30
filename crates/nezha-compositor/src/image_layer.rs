use std::collections::HashMap;

use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, ColorTargetState, ColorWrites, Device, Extent3d,
    FragmentState, MultisampleState, PipelineCompilationOptions, PipelineLayoutDescriptor,
    PrimitiveState, PrimitiveTopology, Queue, RenderPipeline, RenderPipelineDescriptor, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension, VertexState,
};

use crate::layer::{BlendMode, LayerRenderer};
use crate::util::{blend_state_for, compute_scissor_rect};

pub struct ImageLayer {
    pipelines: HashMap<BlendMode, RenderPipeline>,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    texture: Texture,
    texture_view: TextureView,
    sampler: Sampler,
    width: u32,
    height: u32,
}

impl ImageLayer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("image_layer_texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            rgba,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("image_layer_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("image_layer_shader"),
            source: ShaderSource::Wgsl(include_str!("image_layer.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("image_layer_bind_group_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
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

        let bind_group = Self::make_bind_group(device, &bind_group_layout, &texture_view, &sampler);

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("image_layer_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let mut pipelines = HashMap::new();
        for mode in [BlendMode::Normal, BlendMode::Add, BlendMode::Multiply] {
            let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(&format!("image_layer_pipeline_{:?}", mode)),
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

        Self {
            pipelines,
            bind_group_layout,
            bind_group,
            texture,
            texture_view,
            sampler,
            width,
            height,
        }
    }

    pub fn update_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) {
        if width != self.width || height != self.height {
            self.texture = device.create_texture(&TextureDescriptor {
                label: Some("image_layer_texture"),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.texture_view = self.texture.create_view(&TextureViewDescriptor::default());
            self.bind_group = Self::make_bind_group(
                device,
                &self.bind_group_layout,
                &self.texture_view,
                &self.sampler,
            );
            self.width = width;
            self.height = height;
        }

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            rgba,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn make_bind_group(
        device: &Device,
        layout: &BindGroupLayout,
        texture_view: &TextureView,
        sampler: &Sampler,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("image_layer_bind_group"),
            layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

impl LayerRenderer for ImageLayer {
    fn prepare(&mut self, _width: u32, _height: u32, _time: f64) {}

    fn render(&mut self, params: crate::layer::LayerRenderParams<'_>) {
        let (sx, sy, sw, sh) = compute_scissor_rect(params.rect, params.width, params.height);
        let mut pass = params
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("image_layer_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: params.target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: params.load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                multiview_mask: None,
                timestamp_writes: None,
            });
        pass.set_scissor_rect(sx, sy, sw, sh);

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
