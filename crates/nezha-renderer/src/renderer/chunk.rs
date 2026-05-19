use wgpu::*;

use crate::compute::{GpuNote, GpuNoteChunk, KeyInfo};
use crate::keyboard;

use super::Renderer;

/// Maximum GPU note buffer size per chunk (120 MiB).
const MAX_NOTE_BUFFER_BYTES: u64 = 120 * 1024 * 1024;

impl Renderer {
    /// Greedy chunking: group contiguous keys until the note buffer nears limit.
    pub(super) fn chunk_notes(&self, key_notes: &[Vec<GpuNote>; 128]) -> Vec<GpuNoteChunk> {
        let note_size = std::mem::size_of::<GpuNote>() as u64;
        let mut chunks = Vec::new();
        let mut chunk_start: u32 = 0;

        while chunk_start < 128 {
            let mut chunk_notes: Vec<GpuNote> = Vec::new();
            let mut chunk_end = chunk_start;

            // Accumulate keys until the next key would exceed the buffer limit
            while chunk_end < 128 {
                let next_len = key_notes[chunk_end as usize].len();
                let projected = (chunk_notes.len() + next_len) as u64 * note_size;
                if !chunk_notes.is_empty() && projected > MAX_NOTE_BUFFER_BYTES {
                    break;
                }
                chunk_notes.extend_from_slice(&key_notes[chunk_end as usize]);
                chunk_end += 1;
            }
            // Safety: if a single key's notes exceed the limit, we still include it
            if chunk_end == chunk_start {
                chunk_notes.extend_from_slice(&key_notes[chunk_end as usize]);
                chunk_end += 1;
            }

            let key_count = chunk_end - chunk_start;
            let chunk_info = Self::build_chunk_info(chunk_start, chunk_end, key_notes);

            let uniform_buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("chunk_uniforms"),
                size: std::mem::size_of::<crate::compute::ComputeUniforms>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let key_info_buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("key_info"),
                size: (128 * std::mem::size_of::<KeyInfo>()) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&key_info_buf, 0, bytemuck::bytes_of(&chunk_info));

            let notes_buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("notes"),
                size: (chunk_notes.len() * std::mem::size_of::<GpuNote>()) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&notes_buf, 0, bytemuck::cast_slice(&chunk_notes));

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("compute_bind_group"),
                layout: &self.compute.bgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: self.compute.shared_key_layouts_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: key_info_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: notes_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: self.compute.palette_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: self.compute.instance_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 6,
                        resource: self.compute.counter_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 7,
                        resource: self.compute.scan_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 8,
                        resource: self.compute.overflow_buffer.as_entire_binding(),
                    },
                ],
            });

            chunks.push(GpuNoteChunk {
                key_info_buf,
                notes_buf,
                uniform_buf,
                bind_group,
                key_offset: chunk_start,
                key_count,
            });

            chunk_start = chunk_end;
        }

        chunks
    }

    fn build_chunk_info(
        chunk_start: u32,
        chunk_end: u32,
        key_notes: &[Vec<GpuNote>; 128],
    ) -> [KeyInfo; 128] {
        let mut info = [KeyInfo {
            offset: 0,
            count: 0,
            slot: 0,
        }; 128];
        let mut note_offset: u32 = 0;
        let mut white_idx = 0u32;
        let mut black_idx = 75u32;

        for key in chunk_start..chunk_end {
            let n = key_notes[key as usize].len() as u32;
            let slot = if keyboard::is_black_key(key as u8) {
                let s = black_idx;
                black_idx += 1;
                s
            } else {
                let s = white_idx;
                white_idx += 1;
                s
            };
            info[key as usize] = KeyInfo {
                offset: note_offset,
                count: n,
                slot,
            };
            note_offset += n;
        }

        info
    }
}
