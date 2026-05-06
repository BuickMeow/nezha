use wgpu::*;
use nezha_core::MidiFile;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoteInstance {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    scan_indices: [usize; 128],
    last_time: f32,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waterfall_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<NoteInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                    ],
                }],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance_buffer"),
            size: (instance_capacity * std::mem::size_of::<NoteInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            instance_buffer,
            instance_capacity,
            scan_indices: [0; 128],
            last_time: -1.0,
        }
    }

    pub fn render(
        &mut self,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f32,
        midi_file: Option<&MidiFile>,
    ) {
        let uniforms = Uniforms {
            time,
            width: width as f32,
            height: height as f32,
            _pad: 0.0,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        let instances = self.build_instances(width, height, time, midi_file);

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().max(self.instance_capacity * 2);
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance_buffer"),
                size: (self.instance_capacity * std::mem::size_of::<NoteInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if !instances.is_empty() {
            self.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );

            {
                let mut pass = encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: Some("render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw(0..6, 0..instances.len() as u32);
            }
        } else {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    fn build_instances(
        &mut self,
        width: u32,
        height: u32,
        time: f32,
        midi_file: Option<&MidiFile>,
    ) -> Vec<NoteInstance> {
        let midi = match midi_file {
            Some(m) => m,
            None => return Vec::new(),
        };

        let pps = 200.0f32;
        let key_count = 128u8;
        let key_width = width as f32 / key_count as f32;

        let visible_future = height as f32 / pps + 1.0;
        let visible_past = 1.0f32;
        let time_top = time + visible_future;
        let time_bottom = time - visible_past;

        if time < self.last_time {
            self.scan_indices = [0; 128];
        }
        self.last_time = time;

        let estimated = ((width * height) as usize / 4).clamp(50_000, 2_000_000);
        let mut instances = Vec::with_capacity(estimated);

        for key in 0..128u8 {
            let notes = &midi.key_notes[key as usize];
            if notes.is_empty() {
                continue;
            }

            let mut scan = self.scan_indices[key as usize];
            while scan < notes.len() && notes[scan].end < time_bottom {
                scan += 1;
            }
            self.scan_indices[key as usize] = scan;

            let x = key as f32 * key_width;
            let w = key_width;

            for i in scan..notes.len() {
                let note = &notes[i];
                if note.start > time_top {
                    break;
                }

                // 分别取整以避免连续帧间浮点误差产生的抖动
                let start_y = (height as f32 - (note.start - time) * pps).round();
                let end_y = (height as f32 - (note.end - time) * pps).round();
                let y = end_y;
                let h = (start_y - end_y).max(1.0);

                let hue = (key as f32 / 128.0) * 360.0;
                let (r, g, b) = hsv_to_rgb(hue, 0.8, 1.0);

                instances.push(NoteInstance {
                    x,
                    y,
                    w,
                    h,
                    r,
                    g,
                    b,
                    a: 0.9,
                });
            }
        }

        instances
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (r + m, g + m, b + m)
}
