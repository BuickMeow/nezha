use eframe::egui;
use crate::sidebar::SidebarTab;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn is_dark(&self, ctx: &egui::Context) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => ctx.global_style().visuals.dark_mode,
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        match self {
            ThemeMode::Dark => ctx.set_theme(egui::ThemePreference::Dark),
            ThemeMode::Light => ctx.set_theme(egui::ThemePreference::Light),
            ThemeMode::System => ctx.set_theme(egui::ThemePreference::System),
        }
    }
}

pub struct UiState {
    pub active_tab: SidebarTab,
    pub export_format: String,
    pub encoder: String,
    pub export_path: Option<String>,
    pub bg_color: [u8; 3],
    pub note_color: [u8; 3],
    pub theme_mode: ThemeMode,
    pub zoom: f32,
    pub pan_offset: egui::Vec2,
    pub border_width: f32,
    pub rounding: f32,
    pub palette: [[f32; 3]; 16],
}

fn generate_random_palette() -> [[f32; 3]; 16] {
    // 使用 golden ratio 色相分布生成 16 个通道颜色
    let mut colors = [[0.0f32; 3]; 16];
    for i in 0..16 {
        let hue = (i as f32 * 0.618033988749895) % 1.0;
        let (r, g, b) = hsv_to_rgb(hue * 360.0, 0.8, 1.0);
        colors[i] = [r, g, b];
    }
    colors
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r + m, g + m, b + m)
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_tab: SidebarTab::Midi,
            export_format: "MP4".to_string(),
            encoder: "H.264".to_string(),
            export_path: None,
            bg_color: [0, 0, 0],
            note_color: [100, 150, 255],
            theme_mode: ThemeMode::System,
            zoom: 1.0,
            pan_offset: egui::Vec2::ZERO,
            border_width: 0.1,
            rounding: 0.0,
            palette: generate_random_palette(),
        }
    }
}
