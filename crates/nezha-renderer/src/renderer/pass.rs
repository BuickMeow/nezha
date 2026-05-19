use wgpu::*;

use crate::vertex::Uniforms;

use super::Renderer;

impl Renderer {
    pub(super) fn update_palette(&mut self, palette: &[[f32; 3]; 128]) {
        if *palette != self.cache.cached_palette {
            let palette_flat: Vec<f32> = palette
                .iter()
                .flat_map(|c| [c[0], c[1], c[2], 0.0f32])
                .collect();
            self.queue.write_buffer(
                &self.compute.palette_buffer,
                0,
                bytemuck::cast_slice(&palette_flat),
            );
            self.cache.cached_palette = *palette;
        }
    }

    pub(super) fn update_key_layouts(&mut self, width: u32, equal_key_width: bool) {
        if width != self.cache.current_width
            || equal_key_width != self.cache.current_equal_key_width
        {
            Self::update_shared_key_layouts(
                &self.queue,
                &self.compute.shared_key_layouts_buf,
                width,
                equal_key_width,
            );
            self.cache.current_width = width;
            self.cache.current_equal_key_width = equal_key_width;
        }
    }

    pub(super) fn write_render_uniforms(&mut self, time: f64, width: u32, height: u32) {
        let uniforms = Uniforms {
            time: time as f32,
            width: width as f32,
            height: height as f32,
            _pad: 0.0,
        };
        self.queue.write_buffer(
            &self.render.uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );
    }

    pub(super) fn dispatch_compute_pass(
        &self,
        encoder: &mut CommandEncoder,
        note_data_id: Option<usize>,
    ) -> bool {
        let bundle = note_data_id.and_then(|id| self.cache.note_bundles.get(&id));
        match bundle {
            Some(b) if !b.chunks.is_empty() => {
                profile_scope!("compute_pass");
                let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("compute_notes_pass"),
                    timestamp_writes: self.timer.query_set.as_ref().map(|qs| {
                        ComputePassTimestampWrites {
                            query_set: qs,
                            beginning_of_pass_write_index: Some(0),
                            end_of_pass_write_index: Some(1),
                        }
                    }),
                });
                cpass.set_pipeline(&self.compute.pipeline);
                for chunk in &b.chunks {
                    cpass.set_bind_group(0, &chunk.bind_group, &[]);
                    // workgroup_size(64): ceil(key_count / 64) workgroups
                    cpass.dispatch_workgroups((chunk.key_count + 63) / 64, 1, 1);
                }
                drop(cpass);

                let mut finalize_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("finalize_counts_pass"),
                    timestamp_writes: None,
                });
                finalize_pass.set_pipeline(&self.compute.finalize_pipeline);
                finalize_pass.set_bind_group(0, &self.compute.finalize_bind_group, &[]);
                finalize_pass.dispatch_workgroups(1, 1, 1);
                true
            }
            _ => false,
        }
    }

    pub(super) fn execute_render_pass(
        &self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        has_instances: bool,
        draw_keyboard: bool,
        background: [f64; 4],
        clear_background: bool,
    ) {
        profile_scope!("render_pass");

        let load_op = if clear_background {
            LoadOp::Clear(Color {
                r: background[0],
                g: background[1],
                b: background[2],
                a: background[3],
            })
        } else {
            LoadOp::Load
        };

        let mut pass =
            encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("render_pass"),
                timestamp_writes: self.timer.query_set.as_ref().map(|qs| {
                    RenderPassTimestampWrites {
                        query_set: qs,
                        beginning_of_pass_write_index: Some(2),
                        end_of_pass_write_index: Some(3),
                    }
                }),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: load_op,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

        if has_instances {
            pass.set_pipeline(&self.render.pipeline);
            pass.set_bind_group(0, &self.render.bind_group, &[]);
            pass.set_vertex_buffer(0, self.compute.instance_buffer.slice(..));
            pass.draw_indirect(&self.compute.indirect_draw_buffer, 0);
        }

        /// Number of vertices per quad (two triangles).
        const VERTICES_PER_QUAD: u32 = 6;
        /// Number of white-key slots in the keyboard buffer.
        const WHITE_KEY_SLOTS: u32 = 75;
        /// Start slot for black keys.
        const BLACK_KEY_START_SLOT: u32 = 75;

        if draw_keyboard {
            pass.set_pipeline(&self.render.pipeline);
            pass.set_bind_group(0, &self.render.bind_group, &[]);
            pass.set_vertex_buffer(0, self.compute.keyboard_buffer.slice(..));
            pass.draw(0..VERTICES_PER_QUAD, 0..WHITE_KEY_SLOTS);
            pass.draw(0..VERTICES_PER_QUAD, BLACK_KEY_START_SLOT..128);
        }
    }
}
