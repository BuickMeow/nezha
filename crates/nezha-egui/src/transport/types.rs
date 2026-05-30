use eframe::egui;
pub use nezha_compositor::BlendMode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipKind {
    Waterfall,
    SolidColor,
    Counter,
    Audio,
    Image,
    Video,
}

impl ClipKind {
    pub fn is_video_track_kind(&self) -> bool {
        matches!(self, Self::Waterfall | Self::SolidColor | Self::Counter | Self::Image | Self::Video)
    }
}

#[derive(Clone, Debug)]
pub struct LayerCommon {
    pub position_x: f32,
    pub position_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub scale_linked: bool,
    pub blend_mode: BlendMode,
    pub opacity: f32,
}

impl Default for LayerCommon {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_linked: false,
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrackClip {
    pub id: usize,
    pub name: String,
    pub kind: ClipKind,
    pub start: f32,
    pub end: f32,
    pub color: egui::Color32,
    pub text_color: egui::Color32,
    pub template_text: String,
    pub text_alignment: nezha_text::TextAlignment,
    pub thousand_separator: nezha_text::Separator,
    pub zero_padding: bool,
    pub font_name: String,
    pub bold: bool,
    pub bold_offset: f32,
    pub italic: bool,
    pub italic_slant: f32,
    pub outline_enabled: bool,
    pub outline_width: f32,
    pub outline_color: egui::Color32,
    pub letter_spacing: f32,
    pub min_advance: f32,
    pub speed: f32,
    pub border_width: f32,
    pub rounding: f32,
    pub render_mode: nezha_renderer::RenderMode,
    pub equal_key_width: bool,
    pub midi_idx: Option<usize>,
    pub audio_idx: Option<usize>,
    pub media_idx: Option<usize>,
    pub keyboard_height_percent: f32,
    pub font_size: u32,
    pub content_start_offset: u32,
    pub content_end_offset: u32,
    pub song_start_time: f32,
    pub song_duration: f32,
    pub common: LayerCommon,
}

impl TrackClip {
    pub fn new_waterfall(id: usize, midi_idx: Option<usize>) -> Self {
        Self {
            id,
            name: format!("默认瀑布流 {}", id),
            kind: ClipKind::Waterfall,
            start: 0.0,
            end: 0.0,
            color: egui::Color32::from_rgb(80, 150, 220),
            text_color: egui::Color32::WHITE,
            template_text: String::new(),
            text_alignment: nezha_text::TextAlignment::TopLeft,
            thousand_separator: nezha_text::Separator::Comma,
            zero_padding: false,
            font_name: "MiSans".to_string(),
            bold: false,
            bold_offset: 1.0,
            italic: false,
            italic_slant: 0.0,
            outline_enabled: false,
            outline_width: 2.0,
            outline_color: egui::Color32::BLACK,
            letter_spacing: 0.0,
            min_advance: 0.0,
            speed: 1.0,
            border_width: 0.1,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx,
            audio_idx: None,
            media_idx: None,
            keyboard_height_percent: 0.15,
            font_size: 24,
            content_start_offset: 0,
            content_end_offset: 0,
            song_start_time: 0.0,
            song_duration: 0.0,
            common: LayerCommon::default(),
        }
    }

    pub fn new_solid_color(id: usize, color: egui::Color32) -> Self {
        Self {
            id,
            name: format!("纯色 {}", id),
            kind: ClipKind::SolidColor,
            start: 0.0,
            end: 0.0,
            color,
            text_color: egui::Color32::WHITE,
            template_text: String::new(),
            text_alignment: nezha_text::TextAlignment::TopLeft,
            thousand_separator: nezha_text::Separator::Comma,
            zero_padding: false,
            font_name: "MiSans".to_string(),
            bold: false,
            bold_offset: 1.0,
            italic: false,
            italic_slant: 0.0,
            outline_enabled: false,
            outline_width: 2.0,
            outline_color: egui::Color32::BLACK,
            letter_spacing: 0.0,
            min_advance: 0.0,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx: None,
            audio_idx: None,
            media_idx: None,
            keyboard_height_percent: 0.0,
            font_size: 24,
            content_start_offset: 0,
            content_end_offset: 0,
            song_start_time: 0.0,
            song_duration: 0.0,
            common: LayerCommon::default(),
        }
    }

    pub fn new_counter(id: usize, midi_idx: Option<usize>) -> Self {
        Self {
            id,
            name: format!("计数器 {}", id),
            kind: ClipKind::Counter,
            start: 0.0,
            end: 0.0,
            color: egui::Color32::from_rgb(0xBB, 0xB0, 0x94),
            text_color: egui::Color32::WHITE,
            template_text: "Notes: {nc} / {tn}\nBPM: {bpm}\nNPS: {nps}\nPPQ: {ppq}\nPolyphony: {plph}\nTime: {currtime}\nTicks: {currticks}".to_string(),
            text_alignment: nezha_text::TextAlignment::TopLeft,
            thousand_separator: nezha_text::Separator::Comma,
            zero_padding: false,
            font_name: "MiSans".to_string(),
            bold: false,
            bold_offset: 1.0,
            italic: false,
            italic_slant: 0.0,
            outline_enabled: false,
            outline_width: 2.0,
            outline_color: egui::Color32::BLACK,
            letter_spacing: 0.0,
            min_advance: 0.0,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx,
            audio_idx: None,
            media_idx: None,
            keyboard_height_percent: 0.0,
            font_size: 24,
            content_start_offset: 0,
            content_end_offset: 0,
            song_start_time: 0.0,
            song_duration: 0.0,
            common: LayerCommon {
                position_x: 20.0,
                position_y: 20.0,
                ..Default::default()
            },
        }
    }

    pub fn new_audio(id: usize, name: String, audio_idx: usize, duration: f32) -> Self {
        Self {
            id,
            name,
            kind: ClipKind::Audio,
            start: 0.0,
            end: duration,
            color: egui::Color32::from_rgb(100, 200, 100),
            text_color: egui::Color32::WHITE,
            template_text: String::new(),
            text_alignment: nezha_text::TextAlignment::TopLeft,
            thousand_separator: nezha_text::Separator::Comma,
            zero_padding: false,
            font_name: "MiSans".to_string(),
            bold: false,
            bold_offset: 1.0,
            italic: false,
            italic_slant: 0.0,
            outline_enabled: false,
            outline_width: 2.0,
            outline_color: egui::Color32::BLACK,
            letter_spacing: 0.0,
            min_advance: 0.0,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx: None,
            audio_idx: Some(audio_idx),
            media_idx: None,
            keyboard_height_percent: 0.0,
            font_size: 24,
            content_start_offset: 0,
            content_end_offset: 0,
            song_start_time: 0.0,
            song_duration: 0.0,
            common: LayerCommon::default(),
        }
    }

    pub fn new_image(id: usize, name: String, media_idx: usize, duration: f32) -> Self {
        Self {
            id,
            name,
            kind: ClipKind::Image,
            start: 0.0,
            end: duration,
            color: egui::Color32::from_rgb(180, 120, 220),
            text_color: egui::Color32::WHITE,
            template_text: String::new(),
            text_alignment: nezha_text::TextAlignment::TopLeft,
            thousand_separator: nezha_text::Separator::Comma,
            zero_padding: false,
            font_name: "MiSans".to_string(),
            bold: false,
            bold_offset: 1.0,
            italic: false,
            italic_slant: 0.0,
            outline_enabled: false,
            outline_width: 2.0,
            outline_color: egui::Color32::BLACK,
            letter_spacing: 0.0,
            min_advance: 0.0,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx: None,
            audio_idx: None,
            media_idx: Some(media_idx),
            keyboard_height_percent: 0.0,
            font_size: 24,
            content_start_offset: 0,
            content_end_offset: 0,
            song_start_time: 0.0,
            song_duration: 0.0,
            common: LayerCommon::default(),
        }
    }

    pub fn new_video(id: usize, name: String, media_idx: usize, duration: f32) -> Self {
        Self {
            id,
            name,
            kind: ClipKind::Video,
            start: 0.0,
            end: duration,
            color: egui::Color32::from_rgb(220, 160, 60),
            text_color: egui::Color32::WHITE,
            template_text: String::new(),
            text_alignment: nezha_text::TextAlignment::TopLeft,
            thousand_separator: nezha_text::Separator::Comma,
            zero_padding: false,
            font_name: "MiSans".to_string(),
            bold: false,
            bold_offset: 1.0,
            italic: false,
            italic_slant: 0.0,
            outline_enabled: false,
            outline_width: 2.0,
            outline_color: egui::Color32::BLACK,
            letter_spacing: 0.0,
            min_advance: 0.0,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx: None,
            audio_idx: None,
            media_idx: Some(media_idx),
            keyboard_height_percent: 0.0,
            font_size: 24,
            content_start_offset: 0,
            content_end_offset: 0,
            song_start_time: 0.0,
            song_duration: 0.0,
            common: LayerCommon::default(),
        }
    }

    pub fn content_start_time(&self, fps: u32) -> f32 {
        self.start + self.content_start_offset as f32 / fps.max(1) as f32
    }

    pub fn content_end_time(&self, fps: u32) -> f32 {
        self.end - self.content_end_offset as f32 / fps.max(1) as f32
    }

    pub fn update_content_offsets(&mut self, fps: u32) {
        if self.song_duration <= 0.0 {
            return;
        }
        let fps_f = fps.max(1) as f32;
        let song_end_time = self.song_start_time + self.song_duration;
        self.content_start_offset = ((self.song_start_time - self.start).max(0.0) * fps_f).round() as u32;
        self.content_end_offset = ((self.end - song_end_time).max(0.0) * fps_f).round() as u32;
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    pub clips: Vec<TrackClip>,
    pub muted: bool,
    pub solo: bool,
}

impl Track {
    pub fn new_video(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: TrackKind::Video,
            clips: Vec::new(),
            muted: false,
            solo: false,
        }
    }

    pub fn new_audio(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: TrackKind::Audio,
            clips: Vec::new(),
            muted: false,
            solo: false,
        }
    }
}
