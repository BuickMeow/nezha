use eframe::egui;
use nezha_core::is_black_key;
use nezha_renderer::NoteSource;

// ── Appearance constants ─────────────────────────────────────────────────

/// 黑键高度占白键的比例
const BLACK_KEY_HEIGHT_RATIO: f32 = 0.6;

/// 等宽模式下黑键宽度占白键的比例
const EQUAL_BLACK_KEY_WIDTH_RATIO: f32 = 0.7;

/// 真实钢琴比例下黑键宽度占白键的比例
const REALISTIC_BLACK_KEY_WIDTH_RATIO: f32 = 0.65;

/// 黑键水平偏移比例（用于居中黑键）
const BLACK_KEY_OFFSET_RATIO: f32 = 0.5;

/// 白键默认颜色
const WHITE_KEY_DEFAULT: egui::Color32 = egui::Color32::from_rgb(240, 240, 240);
/// 黑键默认颜色
const BLACK_KEY_DEFAULT: egui::Color32 = egui::Color32::from_rgb(40, 40, 42);
/// 白键高亮颜色
const WHITE_KEY_HIGHLIGHT: egui::Color32 = egui::Color32::from_rgb(255, 200, 100);
/// 黑键高亮颜色
const BLACK_KEY_HIGHLIGHT: egui::Color32 = egui::Color32::from_rgb(255, 180, 60);
/// 白键边框颜色
const WHITE_KEY_BORDER: egui::Color32 = egui::Color32::from_rgb(180, 180, 180);
/// 黑键边框颜色
const BLACK_KEY_BORDER: egui::Color32 = egui::Color32::BLACK;

/// 钢琴键盘高度配置：百分比或像素
#[derive(Clone, Debug, PartialEq)]
pub enum KeyboardHeight {
    /// 占预览区高度的百分比 (0.0 ~ 1.0)
    Percent(f32),
    /// 绝对像素值
    Pixels(f32),
}

impl Default for KeyboardHeight {
    fn default() -> Self {
        Self::Percent(0.15)
    }
}

impl KeyboardHeight {
    /// 根据容器总高度计算实际像素值
    pub fn to_pixels(&self, container_height: f32) -> f32 {
        match self {
            Self::Percent(p) => (container_height * p.clamp(0.0, 0.5)).round(),
            Self::Pixels(px) => px.clamp(0.0, container_height * 0.5),
        }
    }
}

/// 钢琴键盘显示范围
#[derive(Clone, Debug)]
pub struct KeyboardRange {
    /// 最低 MIDI 键（默认 0）
    pub min_key: u8,
    /// 最高 MIDI 键（默认 127）
    pub max_key: u8,
}

impl Default for KeyboardRange {
    fn default() -> Self {
        Self {
            min_key: 0,
            max_key: 127,
        }
    }
}

/// 白键在可视范围内的序号（从 min_key 开始计数）
fn white_key_index(key: u8, min_key: u8) -> usize {
    let mut count = 0usize;
    for k in min_key..=key {
        if !is_black_key(k) {
            count += 1;
        }
    }
    count.saturating_sub(1)
}

/// 计算可视范围内白键的总数
fn white_key_count(min_key: u8, max_key: u8) -> usize {
    (min_key..=max_key).filter(|k| !is_black_key(*k)).count()
}

/// 渲染钢琴键盘
///
/// - `ui`: egui 绘制上下文
/// - `rect`: 键盘区域
/// - `active_keys`: 当前激活的键位集合（正在发声的音符的 key）
/// - `active_colors`: 可选，每个激活键的颜色 (key, r, g, b)
/// - `range`: 显示键位范围
/// - `equal_width`: 是否等宽（false = 真实钢琴比例）
pub fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    active_keys: &[u8],
    active_colors: &[(u8, f32, f32, f32)],
    range: &KeyboardRange,
    equal_width: bool,
) {
    let painter = ui.painter_at(rect);
    let w = rect.width();
    let h = rect.height();

    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let min_key = range.min_key;
    let max_key = range.max_key;

    // 背景
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(30, 30, 32));

    if equal_width {
        draw_equal_width_keys(&painter, rect, active_keys, active_colors, min_key, max_key);
    } else {
        draw_realistic_keys(&painter, rect, active_keys, active_colors, min_key, max_key);
    }
}

/// 等宽键位布局
fn draw_equal_width_keys(
    painter: &egui::Painter,
    rect: egui::Rect,
    active_keys: &[u8],
    active_colors: &[(u8, f32, f32, f32)],
    min_key: u8,
    max_key: u8,
) {
    let total_keys = (max_key - min_key + 1) as f32;
    let key_w = rect.width() / total_keys;
    let h = rect.height();
    let white_h = h;
    let black_h = h * BLACK_KEY_HEIGHT_RATIO;

    // 构建激活键查找
    let active_set: std::collections::HashSet<u8> = active_keys.iter().copied().collect();
    let color_map: std::collections::HashMap<u8, (f32, f32, f32)> = active_colors
        .iter()
        .map(|&(k, r, g, b)| (k, (r, g, b)))
        .collect();

    // 先画白键
    for key in min_key..=max_key {
        if is_black_key(key) {
            continue;
        }
        let idx = (key - min_key) as f32;
        let x = rect.min.x + idx * key_w;
        let key_rect =
            egui::Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(key_w, white_h));

        let color = if active_set.contains(&key) {
            if let Some(&(r, g, b)) = color_map.get(&key) {
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            } else {
                WHITE_KEY_HIGHLIGHT
            }
        } else {
            WHITE_KEY_DEFAULT
        };

        painter.rect_filled(key_rect, 1.0, color);
        painter.rect_stroke(
            key_rect,
            1.0,
            egui::Stroke::new(0.5, WHITE_KEY_BORDER),
            egui::StrokeKind::Inside,
        );
    }

    // 再画黑键（覆盖在白键上方）
    for key in min_key..=max_key {
        if !is_black_key(key) {
            continue;
        }
        let idx = (key - min_key) as f32;
        let x = rect.min.x + idx * key_w;
        let black_w = key_w * EQUAL_BLACK_KEY_WIDTH_RATIO;
        let offset_x = (key_w - black_w) / 2.0;
        let key_rect = egui::Rect::from_min_size(
            egui::pos2(x + offset_x, rect.min.y),
            egui::vec2(black_w, black_h),
        );

        let color = if active_set.contains(&key) {
            if let Some(&(r, g, b)) = color_map.get(&key) {
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            } else {
                BLACK_KEY_HIGHLIGHT
            }
        } else {
            BLACK_KEY_DEFAULT
        };

        painter.rect_filled(key_rect, 1.0, color);
        painter.rect_stroke(
            key_rect,
            1.0,
            egui::Stroke::new(0.5, BLACK_KEY_BORDER),
            egui::StrokeKind::Inside,
        );
    }
}

/// 真实钢琴比例键位布局
fn draw_realistic_keys(
    painter: &egui::Painter,
    rect: egui::Rect,
    active_keys: &[u8],
    active_colors: &[(u8, f32, f32, f32)],
    min_key: u8,
    max_key: u8,
) {
    let white_count = white_key_count(min_key, max_key) as f32;
    if white_count <= 0.0 {
        return;
    }
    let white_w = rect.width() / white_count;
    let black_w = white_w * REALISTIC_BLACK_KEY_WIDTH_RATIO;
    let h = rect.height();
    let black_h = h * BLACK_KEY_HEIGHT_RATIO;

    let active_set: std::collections::HashSet<u8> = active_keys.iter().copied().collect();
    let color_map: std::collections::HashMap<u8, (f32, f32, f32)> = active_colors
        .iter()
        .map(|&(k, r, g, b)| (k, (r, g, b)))
        .collect();

    // 先画白键
    for key in min_key..=max_key {
        if is_black_key(key) {
            continue;
        }
        let wi = white_key_index(key, min_key) as f32;
        let x = rect.min.x + wi * white_w;
        let next_x = rect.min.x + (wi + 1.0) * white_w;
        let key_w = (next_x - x).max(1.0);
        let key_rect = egui::Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(key_w, h));

        let color = if active_set.contains(&key) {
            if let Some(&(r, g, b)) = color_map.get(&key) {
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            } else {
                WHITE_KEY_HIGHLIGHT
            }
        } else {
            WHITE_KEY_DEFAULT
        };

        painter.rect_filled(key_rect, 1.0, color);
        painter.rect_stroke(
            key_rect,
            1.0,
            egui::Stroke::new(0.5, WHITE_KEY_BORDER),
            egui::StrokeKind::Inside,
        );
    }

    // 再画黑键
    for key in min_key..=max_key {
        if !is_black_key(key) {
            continue;
        }
        let white_before = white_key_index(key, min_key) as f32;
        let x = rect.min.x + (white_before + 1.0) * white_w - black_w * BLACK_KEY_OFFSET_RATIO;
        let key_rect =
            egui::Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(black_w, black_h));

        let color = if active_set.contains(&key) {
            if let Some(&(r, g, b)) = color_map.get(&key) {
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            } else {
                BLACK_KEY_HIGHLIGHT
            }
        } else {
            BLACK_KEY_DEFAULT
        };

        painter.rect_filled(key_rect, 1.0, color);
        painter.rect_stroke(
            key_rect,
            1.0,
            egui::Stroke::new(0.5, BLACK_KEY_BORDER),
            egui::StrokeKind::Inside,
        );
    }
}

/// 获取当前时间点激活的音符键位列表及其颜色
///
/// 遍历所有 key 的 notes，找到 start <= current_time < end 的音符，
/// 返回 (key, r, g, b) 列表。
pub fn get_active_keys(
    current_time: f64,
    midi: &dyn NoteSource,
    palette: &[[f32; 3]; 128],
) -> Vec<(u8, f32, f32, f32)> {
    let mut active = Vec::new();
    for key in 0..128u8 {
        let notes = midi.key_notes(key);
        let found = notes
            .iter()
            .any(|note| note.start <= current_time && current_time < note.end);
        if found {
            let trk = notes.first().map(|n| n.track as usize % 128).unwrap_or(0);
            let [r, g, b] = palette[trk];
            active.push((key, r, g, b));
        }
    }
    active
}
