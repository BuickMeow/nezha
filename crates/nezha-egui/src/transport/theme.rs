use eframe::egui;

pub struct ThemeColors {
    pub bg: egui::Color32,
    pub video_track_bg: egui::Color32,
    pub audio_track_bg: egui::Color32,
    pub ruler_bg: egui::Color32,
    pub ruler_text: egui::Color32,
    pub ruler_tick: egui::Color32,
    pub playhead: egui::Color32,
    pub text: egui::Color32,
    pub dim_text: egui::Color32,
    pub border: egui::Color32,
    pub header_bg: egui::Color32,
    pub header_bg_muted: egui::Color32,
    pub video_label_bg: egui::Color32,
    pub audio_label_bg: egui::Color32,
    pub scrollbar_bg: egui::Color32,
    pub scrollbar_thumb: egui::Color32,
    pub scrollbar_handle: egui::Color32,
    pub controls_bg: egui::Color32,
    pub btn_mute_off: egui::Color32,
    pub btn_mute_on: egui::Color32,
    pub btn_solo_off: egui::Color32,
    pub btn_solo_on: egui::Color32,
}

impl ThemeColors {
    pub fn new(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    pub fn dark() -> Self {
        Self {
            bg: egui::Color32::from_rgb(28, 28, 28),
            video_track_bg: egui::Color32::from_rgb(40, 44, 52),
            audio_track_bg: egui::Color32::from_rgb(44, 40, 36),
            ruler_bg: egui::Color32::from_rgb(52, 52, 52),
            ruler_text: egui::Color32::from_rgb(220, 220, 220),
            ruler_tick: egui::Color32::from_rgb(100, 100, 100),
            playhead: egui::Color32::from_rgb(255, 90, 90),
            text: egui::Color32::from_rgb(220, 220, 220),
            dim_text: egui::Color32::from_rgb(140, 140, 140),
            border: egui::Color32::from_rgb(60, 60, 60),
            header_bg: egui::Color32::from_rgb(55, 60, 70),
            header_bg_muted: egui::Color32::from_rgb(80, 60, 60),
            video_label_bg: egui::Color32::from_rgb(50, 55, 65),
            audio_label_bg: egui::Color32::from_rgb(55, 50, 45),
            scrollbar_bg: egui::Color32::from_rgb(40, 40, 40),
            scrollbar_thumb: egui::Color32::from_rgb(90, 90, 90),
            scrollbar_handle: egui::Color32::from_rgb(140, 140, 140),
            controls_bg: egui::Color32::from_rgb(35, 35, 35),
            btn_mute_off: egui::Color32::from_rgb(100, 100, 100),
            btn_mute_on: egui::Color32::from_rgb(255, 100, 100),
            btn_solo_off: egui::Color32::from_rgb(100, 100, 100),
            btn_solo_on: egui::Color32::from_rgb(255, 200, 50),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: egui::Color32::from_rgb(245, 245, 245),
            video_track_bg: egui::Color32::from_rgb(230, 233, 240),
            audio_track_bg: egui::Color32::from_rgb(240, 235, 230),
            ruler_bg: egui::Color32::from_rgb(220, 220, 220),
            ruler_text: egui::Color32::from_rgb(50, 50, 50),
            ruler_tick: egui::Color32::from_rgb(130, 130, 130),
            playhead: egui::Color32::from_rgb(220, 60, 60),
            text: egui::Color32::from_rgb(40, 40, 40),
            dim_text: egui::Color32::from_rgb(120, 120, 120),
            border: egui::Color32::from_rgb(180, 180, 180),
            header_bg: egui::Color32::from_rgb(210, 215, 225),
            header_bg_muted: egui::Color32::from_rgb(220, 190, 190),
            video_label_bg: egui::Color32::from_rgb(215, 220, 230),
            audio_label_bg: egui::Color32::from_rgb(230, 225, 220),
            scrollbar_bg: egui::Color32::from_rgb(220, 220, 220),
            scrollbar_thumb: egui::Color32::from_rgb(160, 160, 160),
            scrollbar_handle: egui::Color32::from_rgb(120, 120, 120),
            controls_bg: egui::Color32::from_rgb(235, 235, 235),
            btn_mute_off: egui::Color32::from_rgb(180, 180, 180),
            btn_mute_on: egui::Color32::from_rgb(255, 100, 100),
            btn_solo_off: egui::Color32::from_rgb(180, 180, 180),
            btn_solo_on: egui::Color32::from_rgb(255, 180, 40),
        }
    }
}
