use crate::compute::ComputeUniforms;
use crate::keyboard;
use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::{RenderMode, RenderStyle};

use super::Renderer;

pub(super) struct PreparedFrame {
    pub(super) scroll_tick: f32,
    pub(super) base_uniforms: ComputeUniforms,
    pub(super) draw_keyboard: bool,
    pub(super) keyboard_changed: bool,
}

impl Renderer {
    pub(super) fn prepare_frame(
        &self,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: Option<&dyn NoteSource>,
        style: &RenderStyle,
    ) -> PreparedFrame {
        let mode = match style.render_mode {
            RenderMode::TimeBased => 0,
            RenderMode::TickBased => 1,
        };
        let ticks_per_beat = midi.and_then(|source| source.ticks_per_beat()).unwrap_or(480) as f32;
        let scroll_tick = midi
            .and_then(|source| source.tick_at_time(time))
            .unwrap_or(time * ticks_per_beat as f64 * 2.0) as f32;
        let draw_keyboard = style.keyboard_height > 0.0 && midi.is_some();
        let keyboard_changed = self.cache.is_keyboard_state_changed(
            draw_keyboard,
            time,
            scroll_tick as f64,
            style.keyboard_height,
            width,
            style.equal_key_width,
        );

        PreparedFrame {
            scroll_tick,
            base_uniforms: ComputeUniforms {
                time: time as f32,
                scroll_tick,
                width: width as f32,
                height: height as f32,
                speed,
                keyboard_height: style.keyboard_height,
                border_width: style.border_width,
                rounding: style.rounding,
                mode,
                ticks_per_beat,
                equal_key_width: if style.equal_key_width { 1 } else { 0 },
                key_offset: 0,
                key_count: 0,
            },
            draw_keyboard,
            keyboard_changed,
        }
    }

    pub(super) fn write_chunk_uniforms(
        &self,
        note_data_id: Option<usize>,
        base_uniforms: ComputeUniforms,
    ) {
        if let Some(bundle) = note_data_id.and_then(|id| self.cache.note_bundles.get(&id)) {
            for chunk in &bundle.chunks {
                let uniforms = ComputeUniforms {
                    key_offset: chunk.key_offset,
                    key_count: chunk.key_count,
                    ..base_uniforms
                };
                self.queue
                    .write_buffer(&chunk.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
            }
        }
    }

    pub(super) fn update_keyboard_instances(
        &mut self,
        width: u32,
        height: u32,
        time: f64,
        scroll_tick: f64,
        midi: &dyn NoteSource,
        style: &RenderStyle,
        render_state: &MidiRenderState,
    ) {
        let instances = keyboard::build_keyboard_instances(
            width,
            height,
            time,
            midi,
            style.keyboard_height,
            style.equal_key_width,
            &style.palette,
            render_state,
        );
        self.queue.write_buffer(
            &self.compute.keyboard_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );
        self.cache
            .mark_keyboard_clean(time, scroll_tick, style.keyboard_height);
    }
}
