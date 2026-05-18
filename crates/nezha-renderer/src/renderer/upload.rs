use wgpu::{Buffer, Queue};

use crate::compute::{GpuNote, GpuNoteBundle};
use crate::source::NoteSource;

use super::Renderer;

impl Renderer {
    /// Upload note data from a [`NoteSource`] to the GPU.
    /// Notes are automatically split into chunks to stay within GPU buffer limits.
    pub fn upload_note_data(
        &mut self,
        id: usize,
        source: &dyn NoteSource,
        width: u32,
        equal_key_width: bool,
    ) {
        profile_scope!("upload_note_data");
        Self::update_shared_key_layouts(
            &self.queue,
            &self.compute.shared_key_layouts_buf,
            width,
            equal_key_width,
        );
        self.cache.current_width = width;
        self.cache.current_equal_key_width = equal_key_width;

        let mut key_notes: [Vec<GpuNote>; 128] = std::array::from_fn(|_| Vec::new());
        for key in 0..128u8 {
            let notes = source.key_notes(key);
            for note in notes {
                key_notes[key as usize].push(GpuNote {
                    start: note.start as f32,
                    end: note.end as f32,
                    start_tick: note.start_tick,
                    end_tick: note.end_tick,
                    track: note.track as u32,
                    velocity: note.velocity as u32,
                });
            }
        }

        let chunks = self.chunk_notes(&key_notes);
        self.cache.note_bundles.insert(id, GpuNoteBundle { chunks });
    }

    /// Remove previously uploaded note data by its ID.
    pub fn remove_note_data(&mut self, id: usize) {
        self.cache.note_bundles.remove(&id);
        self.cache.keyboard_dirty = true;
    }

    pub fn clear_note_data(&mut self) {
        self.cache.clear_note_data();
    }

    pub(super) fn update_shared_key_layouts(
        queue: &Queue,
        buf: &Buffer,
        width: u32,
        equal_key_width: bool,
    ) {
        let layouts = crate::keyboard::compute_key_layouts(width, equal_key_width);
        let layout_data: Vec<f32> = layouts.iter().flat_map(|(x, w)| [*x, *w]).collect();
        queue.write_buffer(buf, 0, bytemuck::cast_slice(&layout_data));
    }
}
