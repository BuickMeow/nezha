use std::sync::mpsc;

pub(crate) enum MidiLoadEvent {
    /// 解析音轨的进度。
    Progress(nezha_core::LoadProgress),
    /// 自定义状态文本（用于 DMS 解压等非音轨阶段）。
    Status(String),
    Complete(Box<Result<nezha_core::MidiFile, nezha_core::MidiError>>),
}

pub(crate) struct MidiLoader {
    pub path: String,
    pub rx: mpsc::Receiver<MidiLoadEvent>,
    pub current_progress: Option<nezha_core::LoadProgress>,
    pub status_message: Option<String>,
}
