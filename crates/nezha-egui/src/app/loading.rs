use super::App;
use eframe::egui;
use std::sync::mpsc;

pub(super) enum MidiLoadEvent {
    Progress(nezha_core::LoadProgress),
    Complete(Result<nezha_core::MidiFile, nezha_core::MidiError>),
}

pub(super) struct MidiLoader {
    pub(super) path: String,
    pub(super) rx: mpsc::Receiver<MidiLoadEvent>,
    pub(super) current_progress: Option<nezha_core::LoadProgress>,
}

// ------------------------------------------------------------------
// Archive picker
// ------------------------------------------------------------------

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
    pub fn pick_midi_file(&mut self) {
        if self.midi_loader.is_some() || self.archive_picker.is_some() {
            return;
        }

        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "MIDI / 压缩包",
                &[
                    "mid", "midi", "zip", "7z", "tar", "tar.gz", "tgz", "tar.xz", "txz",
                ],
            )
            .pick_file()
        {
            let path_str = path.to_string_lossy().to_string();

            if is_archive_file(&path_str) {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let result = nezha_archive::Archive::open(&path).map(|archive| {
                            let entries = archive.list_midi_files();
                            (archive, entries)
                        });
                        let _ = tx.send(result.map_err(|e| e.to_string()));
                    }
                });

                self.archive_picker = Some(ArchivePickerState::Opening { path: path_str, rx });
            } else {
                let (tx, rx) = mpsc::channel();

                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let result = nezha_core::MidiFile::load_with_progress(&path, |progress| {
                            let _ = tx.send(MidiLoadEvent::Progress(progress));
                        });
                        let _ = tx.send(MidiLoadEvent::Complete(result));
                    }
                });

                self.midi_loader = Some(MidiLoader {
                    path: path_str,
                    rx,
                    current_progress: None,
                });
            }
        }
    }

    pub(super) fn show_midi_loading(&mut self, ui: &mut egui::Ui) {
        if let Some(mut loader) = self.midi_loader.take() {
            let mut done = false;
            while let Ok(event) = loader.rx.try_recv() {
                match event {
                    MidiLoadEvent::Progress(progress) => loader.current_progress = Some(progress),
                    MidiLoadEvent::Complete(result) => {
                        match result {
                            Ok(midi) => {
                                let path = loader.path.clone();
                                self.project.insert_midi(path, midi);
                                self.render_ctx.reset_midi_state();
                            }
                            Err(error) => {
                                self.project.last_error = Some(format!("MIDI 加载失败: {}", error));
                            }
                        }
                        done = true;
                        break;
                    }
                }
            }

            if !done {
                self.midi_loader = Some(loader);
            }
        }

        if let Some(loader) = &self.midi_loader {
            let screen_rect = ui.ctx().content_rect();
            ui.ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    "midi_loading_overlay".into(),
                ))
                .rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
                );

            egui::Window::new("正在加载 MIDI")
                .order(egui::Order::Tooltip)
                .collapsible(false)
                .resizable(false)
                .movable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    if let Some(progress) = &loader.current_progress {
                        ui.label(format!(
                            "正在解析音轨 {} / {}",
                            progress.current_track, progress.total_tracks
                        ));
                        let ratio =
                            progress.current_track as f32 / progress.total_tracks.max(1) as f32;
                        ui.add(egui::ProgressBar::new(ratio).show_percentage());
                    } else {
                        ui.label("正在读取文件...");
                        ui.add(egui::Spinner::new());
                    }
                });
        }
    }

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
            .default_size([420.0, 500.0])
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
                                            let response = ui
                                                .horizontal(|ui| {
                                                    let text = if is_selected {
                                                        format!("▶ {}", entry.name)
                                                    } else {
                                                        format!("  {}", entry.name)
                                                    };
                                                    let label = egui::Label::new(
                                                        egui::RichText::new(&text)
                                                            .monospace()
                                                            .color(if is_selected {
                                                                ui.visuals().selection.bg_fill
                                                            } else {
                                                                ui.visuals().text_color()
                                                            }),
                                                    )
                                                    .selectable(false)
                                                    .sense(egui::Sense::click());
                                                    ui.add(label);

                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.label(
                                                                egui::RichText::new(&size_text)
                                                                    .monospace()
                                                                    .color(
                                                                        ui.visuals()
                                                                            .weak_text_color(),
                                                                    ),
                                                            );
                                                        },
                                                    );
                                                })
                                                .response;

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

fn is_archive_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".zip")
        || lower.ends_with(".7z")
        || lower.ends_with(".tar")
        || lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".tar.xz")
        || lower.ends_with(".txz")
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
