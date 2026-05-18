use crate::compute::GpuNoteBundle;
use std::collections::HashMap;

pub(super) struct RendererCache {
    pub(super) note_bundles: HashMap<usize, GpuNoteBundle>,
    pub(super) current_width: u32,
    pub(super) current_equal_key_width: bool,
    pub(super) cached_palette: [[f32; 3]; 128],
    pub(super) keyboard_dirty: bool,
    pub(super) cached_keyboard_time: f64,
    pub(super) cached_scroll_tick: f64,
    pub(super) cached_keyboard_height: f32,
}

impl Default for RendererCache {
    fn default() -> Self {
        Self {
            note_bundles: HashMap::new(),
            current_width: 0,
            current_equal_key_width: false,
            cached_palette: [[0.0; 3]; 128],
            keyboard_dirty: true,
            cached_keyboard_time: f64::NEG_INFINITY,
            cached_scroll_tick: f64::NEG_INFINITY,
            cached_keyboard_height: -1.0,
        }
    }
}

impl RendererCache {
    pub(super) fn is_keyboard_state_changed(
        &self,
        draw_keyboard: bool,
        time: f64,
        scroll_tick: f64,
        keyboard_height: f32,
        width: u32,
        equal_key_width: bool,
    ) -> bool {
        draw_keyboard
            && (self.keyboard_dirty
                || (time - self.cached_keyboard_time).abs() > f64::EPSILON
                || (scroll_tick - self.cached_scroll_tick).abs() > f64::EPSILON
                || (keyboard_height - self.cached_keyboard_height).abs() > f32::EPSILON
                || width != self.current_width
                || equal_key_width != self.current_equal_key_width)
    }

    pub(super) fn mark_keyboard_clean(&mut self, time: f64, scroll_tick: f64, keyboard_height: f32) {
        self.keyboard_dirty = false;
        self.cached_keyboard_time = time;
        self.cached_scroll_tick = scroll_tick;
        self.cached_keyboard_height = keyboard_height;
    }

    pub(super) fn clear_note_data(&mut self) {
        self.note_bundles.clear();
        self.keyboard_dirty = true;
    }
}
