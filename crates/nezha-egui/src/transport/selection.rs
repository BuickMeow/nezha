use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct TimelineSelection {
    pub selected_ids: HashSet<usize>,
}

impl TimelineSelection {
    /// 替换选择：清空旧选，选中新 clip
    pub fn select(&mut self, clip_id: usize) {
        self.selected_ids.clear();
        self.selected_ids.insert(clip_id);
    }

    /// 切换选择状态（Ctrl/Cmd+点击）
    pub fn toggle(&mut self, clip_id: usize) {
        if !self.selected_ids.remove(&clip_id) {
            self.selected_ids.insert(clip_id);
        }
    }

    /// 追加到选择（Shift+点击）
    pub fn add(&mut self, clip_id: usize) {
        self.selected_ids.insert(clip_id);
    }

    /// 批量设置选择（框选结果）
    pub fn set_bulk(&mut self, ids: HashSet<usize>) {
        self.selected_ids = ids;
    }

    pub fn clear(&mut self) {
        self.selected_ids.clear();
    }

    pub fn contains(&self, clip_id: usize) -> bool {
        self.selected_ids.contains(&clip_id)
    }

    pub fn is_empty(&self) -> bool {
        self.selected_ids.is_empty()
    }

    /// 返回第一个选中的 id（用于属性面板等向后兼容场景）
    pub fn primary_selected(&self) -> Option<usize> {
        self.selected_ids.iter().next().copied()
    }

    pub fn selected_count(&self) -> usize {
        self.selected_ids.len()
    }
}
