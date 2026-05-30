#[derive(Clone, Debug, Default)]
pub struct TimelineSelection {
    pub selected_clip_id: Option<usize>,
}

impl TimelineSelection {
    pub fn select(&mut self, clip_id: usize) {
        self.selected_clip_id = Some(clip_id);
    }

    pub fn clear(&mut self) {
        self.selected_clip_id = None;
    }
}
