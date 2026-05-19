use super::App;
use eframe::egui;
use std::sync::mpsc;

use super::loading::{MidiLoadEvent, MidiLoader};

pub(super) struct ArchivePicker {
    pub(super) path: String,
    pub(super) archive: nezha_archive::Archive,
    pub(super) entries: Vec<nezha_archive::ArchiveEntry>,
    pub(super) selected_idx: Option<usize>,
    pub(super) search_query: String,
    pub(super) filtered: Vec<usize>,
}

impl ArchivePicker {
    fn recompute_filter(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered.clear();
        for (idx, entry) in self.entries.iter().enumerate() {
            if query.is_empty() || entry.name.to_lowercase().contains(&query) {
                self.filtered.push(idx);
            }
        }
    }
}

pub(super) enum ArchivePickerState {
    Opening {
        path: String,
        rx: mpsc::Receiver<
            Result<(nezha_archive::Archive, Vec<nezha_archive::ArchiveEntry>), String>,
        >,
    },
    Opened(ArchivePicker),
}

impl App {
    pub(super) fn show_archive_picker(&mut self, ui: &mut egui::Ui) {
        // 1) 处理 Opening 状态：检查后台线程是否完成
        if let Some(ArchivePickerState::Opening { path, rx }) = &self.archive_picker {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok((archive, entries)) => {
                        if entries.is_empty() {
                            self.project.last_error =
                                Some("压缩包内没有找到 MIDI 文件".to_string());
                            self.archive_picker = None;
                            return;
                        }
                        // 只有一个 MIDI 文件时直接加载，跳过选择对话框
                        if entries.len() == 1 {
                            let name = entries[0].name.clone();
                            match archive.read_file(&name) {
                                Ok(bytes) => {
                                    let display_path = format!("{path} > {name}");
                                    let (tx, rx) = mpsc::channel();
                                    std::thread::spawn(move || {
                                        let result =
                                            nezha_core::MidiFile::load_from_bytes_with_progress(
                                                &bytes,
                                                |progress| {
                                                    let _ =
                                                        tx.send(MidiLoadEvent::Progress(progress));
                                                },
                                            );
                                        let _ = tx.send(MidiLoadEvent::Complete(result));
                                    });
                                    self.midi_loader = Some(MidiLoader {
                                        path: display_path,
                                        rx,
                                        current_progress: None,
                                    });
                                }
                                Err(e) => {
                                    self.project.last_error =
                                        Some(format!("读取压缩包内文件失败: {}", e));
                                }
                            }
                            self.archive_picker = None;
                            return;
                        }
                        let mut picker = ArchivePicker {
                            path: path.clone(),
                            archive,
                            entries,
                            selected_idx: None,
                            search_query: String::new(),
                            filtered: Vec::new(),
                        };
                        picker.recompute_filter();
                        self.archive_picker = Some(ArchivePickerState::Opened(picker));
                    }
                    Err(e) => {
                        self.project.last_error = Some(format!("压缩包打开失败: {}", e));
                        self.archive_picker = None;
                        return;
                    }
                }
            }
        }

        // 2) 如果没有 picker 了，直接返回
        let Some(state) = &mut self.archive_picker else {
            return;
        };

        // 如果还在 Opening，显示 loading 遮罩
        let picker = match state {
            ArchivePickerState::Opening { .. } => {
                let screen_rect = ui.ctx().content_rect();
                ui.ctx()
                    .layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        "archive_picker_overlay".into(),
                    ))
                    .rect_filled(
                        screen_rect,
                        0.0,
                        egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
                    );

                egui::Window::new("正在读取压缩包...")
                    .order(egui::Order::Tooltip)
                    .collapsible(false)
                    .resizable(false)
                    .movable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(ui.ctx(), |ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.label("正在扫描压缩包内的 MIDI 文件...");
                        });
                    });
                return;
            }
            ArchivePickerState::Opened(p) => p,
        };

        // 3) 渲染选择对话框
        let screen_rect = ui.ctx().content_rect();
        ui.ctx()
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                "archive_picker_overlay".into(),
            ))
            .rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
            );

        let mut confirmed = false;
        let mut cancelled = false;

        egui::Window::new("📦 从压缩包中选择 MIDI 文件")
            .order(egui::Order::Tooltip)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_size([560.0, 500.0])
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "来源: {}",
                    std::path::Path::new(&picker.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&picker.path)
                ));
                ui.add_space(4.0);

                // 搜索框
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut picker.search_query)
                            .hint_text("搜索文件名...")
                            .desired_width(ui.available_width()),
                    );
                    if response.changed() {
                        picker.recompute_filter();
                    }
                });

                ui.add_space(4.0);

                // 文件列表
                let row_height = 24.0;
                let available_height = ui.available_height() - 48.0;

                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .show(ui, |ui| {
                        if picker.filtered.is_empty() {
                            ui.add_space(available_height.max(200.0) / 2.0 - 12.0);
                            ui.horizontal_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("没有匹配的文件")
                                        .color(ui.visuals().weak_text_color()),
                                );
                            });
                        } else {
                            egui::ScrollArea::vertical()
                                .max_height(available_height.max(200.0))
                                .show_rows(
                                    ui,
                                    row_height,
                                    picker.filtered.len(),
                                    |ui, row_range| {
                                        for i in row_range {
                                            let entry_idx = picker.filtered[i];
                                            let entry = &picker.entries[entry_idx];
                                            let is_selected =
                                                picker.selected_idx == Some(entry_idx);

                                            let size_text = format_size(entry.size);
                                            let text = if is_selected {
                                                format!("▶ {}", entry.name)
                                            } else {
                                                format!("  {}", entry.name)
                                            };

                                            // 分配整行可点击区域
                                            let full_width = ui.available_width();
                                            let (rect, response) = ui.allocate_at_least(
                                                egui::vec2(full_width, row_height),
                                                egui::Sense::click(),
                                            );

                                            if ui.is_rect_visible(rect) {
                                                // 选中/悬停背景
                                                if is_selected {
                                                    ui.painter().rect_filled(
                                                        rect,
                                                        egui::CornerRadius::ZERO,
                                                        ui.visuals()
                                                            .selection
                                                            .bg_fill
                                                            .gamma_multiply(0.25),
                                                    );
                                                } else if response.hovered() {
                                                    ui.painter().rect_filled(
                                                        rect,
                                                        egui::CornerRadius::ZERO,
                                                        ui.visuals().widgets.hovered.weak_bg_fill,
                                                    );
                                                }

                                                // 精确布局：左侧文件名（截断）+ 间距 + 右侧大小
                                                let gap = 10.0;
                                                let size_width = 72.0;
                                                let h_padding = 8.0;

                                                let mut row_ui =
                                                    ui.new_child(
                                                        egui::UiBuilder::new()
                                                            .max_rect(rect.shrink2(egui::vec2(
                                                                h_padding, 0.0,
                                                            )))
                                                            .layout(egui::Layout::left_to_right(
                                                                egui::Align::Center,
                                                            )),
                                                    );

                                                let name_width =
                                                    (row_ui.available_width() - gap - size_width)
                                                        .max(0.0);

                                                row_ui.add_sized(
                                                    egui::vec2(name_width, row_height),
                                                    egui::Label::new(
                                                        egui::RichText::new(&text).color(
                                                            if is_selected {
                                                                ui.visuals().selection.bg_fill
                                                            } else {
                                                                ui.visuals().text_color()
                                                            },
                                                        ),
                                                    )
                                                    .selectable(false)
                                                    .truncate(),
                                                );

                                                row_ui.add_space(gap);

                                                row_ui.allocate_ui_with_layout(
                                                    egui::vec2(size_width, row_height),
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(&size_text).color(
                                                                ui.visuals().weak_text_color(),
                                                            ),
                                                        );
                                                    },
                                                );
                                            }

                                            if response.clicked() {
                                                picker.selected_idx = Some(entry_idx);
                                            }
                                        }
                                    },
                                );
                        }
                    });

                ui.add_space(8.0);

                // 底部按钮
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let count = picker.filtered.len();
                        ui.label(
                            egui::RichText::new(format!("共 {count} 个 MIDI 文件"))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let has_selection = picker.selected_idx.is_some();
                        let confirm_btn =
                            ui.add_enabled(has_selection, egui::Button::new("确认导入"));
                        if confirm_btn.clicked() {
                            confirmed = true;
                        }

                        if ui.button("取消").clicked() {
                            cancelled = true;
                        }
                    });
                });
            });

        if cancelled {
            self.archive_picker = None;
            return;
        }

        if confirmed {
            if let Some(ArchivePickerState::Opened(picker)) = self.archive_picker.take() {
                if let Some(idx) = picker.selected_idx {
                    let entry = &picker.entries[idx];
                    match picker.archive.read_file(&entry.name) {
                        Ok(bytes) => {
                            let display_path = format!("{} > {}", picker.path, entry.name);
                            let (tx, rx) = mpsc::channel();

                            std::thread::spawn(move || {
                                let result = nezha_core::MidiFile::load_from_bytes_with_progress(
                                    &bytes,
                                    |progress| {
                                        let _ = tx.send(MidiLoadEvent::Progress(progress));
                                    },
                                );
                                let _ = tx.send(MidiLoadEvent::Complete(result));
                            });

                            self.midi_loader = Some(MidiLoader {
                                path: display_path,
                                rx,
                                current_progress: None,
                            });
                        }
                        Err(e) => {
                            self.project.last_error = Some(format!("读取压缩包内文件失败: {}", e));
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn is_archive_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".zip")
        || lower.ends_with(".7z")
        || lower.ends_with(".tar")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".tar.xz")
        || lower.ends_with(".txz")
}

pub(super) fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
