use eframe::egui;

pub fn snap_to_frame(time: f32, fps: u32) -> f32 {
    if fps == 0 {
        return time.max(0.0);
    }
    let frame = (time * fps as f32).round();
    (frame / fps as f32).max(0.0)
}

pub fn format_timecode_frames(time: f32, fps: u32) -> String {
    let total_frames = (time * fps.max(1) as f32).round() as u32;
    let frames = total_frames % fps.max(1);
    let total_seconds = total_frames / fps.max(1);
    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    format!("{:02}:{:02}:{:02}", minutes, seconds, frames)
}

pub fn format_timecode_seconds(time: f32) -> String {
    let min = (time as u32) / 60;
    let sec = (time as u32) % 60;
    format!("{}:{:02}", min, sec)
}

pub fn format_timecode_full(time: f32, fps: u32) -> String {
    let total_frames = (time * fps.max(1) as f32).round() as u32;
    let frames = total_frames % fps.max(1);
    let total_seconds = total_frames / fps.max(1);
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let hours = total_minutes / 60;
    format!("{:02}:{:02}:{:02}:{:02}", hours, minutes, seconds, frames)
}

pub fn font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Proportional)
}
