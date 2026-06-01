mod export;
pub mod media;
pub mod project;
mod settings;
mod style;

use crate::app::ThemeMode;
use crate::app::project_state::{MidiEntry, SoundFontEntry};
use crate::sidebar::SidebarTab;
use eframe::egui;
use rust_i18n::t;

pub struct ConfigState<'a> {
    pub active_tab: SidebarTab,
    pub midi_files: &'a [MidiEntry],
    pub highlighted_midi_idx: &'a mut Option<usize>,
    pub render_width: &'a mut u32,
    pub render_height: &'a mut u32,
    pub fps: &'a mut u32,
    pub export_format: &'a mut String,
    pub encoder: &'a mut String,
    pub encoder_backend: &'a mut String,
    pub export_path: &'a mut Option<String>,
    pub theme_mode: &'a mut ThemeMode,
    pub locale: &'a mut String,
    pub soundfonts: &'a [SoundFontEntry],
    pub audio_device_name: &'a mut Option<String>,
    pub audio_devices: &'a [String],
    pub media: &'a mut crate::app::project_state::MediaStore,
    pub timeline: &'a mut crate::transport::TimelineState,
    pub audio: &'a mut crate::app::project_state::AudioStore,
}

#[derive(Clone, Debug)]
pub enum ConfigAction {
    SelectMidi,
    AddWaterfall,
    AddSolidColor,
    AddCounter,
    RemoveMidi(usize),
    StartExport,
    AddSoundfont,
    RemoveSoundfont(usize),
    MoveSoundfontUp(usize),
    MoveSoundfontDown(usize),
    RenderAudio(usize),
    ImportMedia,
    AddMediaToTimeline(usize),
    RemoveMedia(usize),
    LocaleChanged,
}

pub fn show(ui: &mut egui::Ui, state: &mut ConfigState) -> Option<ConfigAction> {
    let mut action = None;

    egui::ScrollArea::vertical()
        .id_salt("config_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading(t!("config.title"));
            ui.separator();

            let result: Option<ConfigAction> = match state.active_tab {
                SidebarTab::Style => style::show(ui, state.midi_files, state.highlighted_midi_idx),
                SidebarTab::Project => match project::show(
                    ui,
                    state.render_width,
                    state.render_height,
                    state.fps,
                    state.soundfonts,
                ) {
                    Some(pa) => {
                        use project::ProjectAction;
                        match pa {
                            ProjectAction::AddSoundfont(_) => Some(ConfigAction::AddSoundfont),
                            ProjectAction::RemoveSoundfont(i) => {
                                Some(ConfigAction::RemoveSoundfont(i))
                            }
                            ProjectAction::MoveSoundfontUp(i) => {
                                Some(ConfigAction::MoveSoundfontUp(i))
                            }
                            ProjectAction::MoveSoundfontDown(i) => {
                                Some(ConfigAction::MoveSoundfontDown(i))
                            }
                        }
                    }
                    None => None,
                },
                SidebarTab::Media => media::show(ui, state.media, state.timeline, state.audio),
                SidebarTab::Export => export::show(
                    ui,
                    state.export_format,
                    state.encoder,
                    state.encoder_backend,
                    state.export_path,
                    state.midi_files,
                ),
                SidebarTab::Settings => {
                    settings::show(
                        ui,
                        state.theme_mode,
                        state.locale,
                        state.audio_device_name,
                        state.audio_devices,
                    )
                }
            };

            if let Some(a) = result {
                action = Some(a);
            }
        });

    action
}
