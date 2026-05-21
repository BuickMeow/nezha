//! 配置面板入口。
//!
//! 根据激活的侧边栏标签页，委托给对应的子模块渲染 UI。
//!
//! 子模块位于 `config_panel/` 目录（Rust 2018+ 约定）。

mod export;
mod project;
mod settings;
mod style;

use crate::app::ThemeMode;
use crate::app::project_state::MidiEntry;
use crate::sidebar::SidebarTab;
use eframe::egui;

/// Truncate a path string by keeping the filename intact and
/// truncating the directory portion with an ellipsis in the middle.
/// e.g. "/very/long/directory/structure/file.mid" → "/very/.../file.mid"
pub(crate) fn truncate_path(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let path = std::path::Path::new(s);
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

    if file_name.is_empty() {
        s.chars().take(max_chars - 1).collect::<String>() + "…"
    } else if parent.is_empty() {
        let keep = max_chars.saturating_sub(1);
        if file_name.chars().count() <= keep {
            file_name.to_string()
        } else {
            file_name.chars().take(keep - 1).collect::<String>() + "…"
        }
    } else {
        let suffix_len = file_name.chars().count() + 2;
        if suffix_len >= max_chars {
            let keep = max_chars.saturating_sub(1);
            if file_name.chars().count() <= keep {
                file_name.to_string()
            } else {
                file_name.chars().take(keep - 1).collect::<String>() + "…"
            }
        } else {
            let prefix_len = max_chars - suffix_len;
            let prefix: String = parent.chars().take(prefix_len).collect();
            format!("{}…/{}", prefix, file_name)
        }
    }
}

pub struct ConfigState<'a> {
    pub active_tab: SidebarTab,
    pub midi_files: &'a [MidiEntry],
    pub highlighted_midi_idx: &'a mut Option<usize>,
    pub render_width: &'a mut u32,
    pub render_height: &'a mut u32,
    pub fps: &'a mut u32,
    pub export_format: &'a mut String,
    pub encoder: &'a mut String,
    pub export_path: &'a mut Option<String>,
    pub theme_mode: &'a mut ThemeMode,
}

#[derive(Clone, Debug)]
pub enum ConfigAction {
    SelectMidi,
    AddWaterfall,
    AddSolidColor,
    AddCounter,
    RemoveMidi(usize),
    StartExport,
}

pub fn show(ui: &mut egui::Ui, state: &mut ConfigState) -> Option<ConfigAction> {
    let mut action = None;

    egui::ScrollArea::vertical()
        .id_salt("config_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("配置");
            ui.separator();

            let result = match state.active_tab {
                SidebarTab::Style => style::show(ui, state.midi_files, state.highlighted_midi_idx),
                SidebarTab::Project => {
                    project::show(ui, state.render_width, state.render_height, state.fps);
                    None
                }
                SidebarTab::Export => export::show(
                    ui,
                    state.export_format,
                    state.encoder,
                    state.export_path,
                    state.midi_files,
                ),
                SidebarTab::Settings => {
                    settings::show(ui, state.theme_mode);
                    None
                }
            };
            action = result;
        });

    action
}
