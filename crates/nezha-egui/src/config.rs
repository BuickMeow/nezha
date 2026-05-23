use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::ThemeMode;
use crate::app::project_state::SoundFontEntry;

/// Persisted application configuration.
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub soundfont_paths: Vec<PathBuf>,
    pub audio_device_name: Option<String>,
    pub theme_mode: String, // "dark", "light", "system"
    pub render_width: u32,
    pub render_height: u32,
    pub fps: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: String, // "stereo", "mono"
    pub audio_use_limiter: bool,
    pub audio_layers: u32,
    pub audio_min_velocity: u8,
    pub export_format: String,
    pub encoder: String,
    pub encoder_backend: String,
    pub export_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            soundfont_paths: Vec::new(),
            audio_device_name: None,
            theme_mode: "dark".to_string(),
            render_width: 1920,
            render_height: 1080,
            fps: 60,
            audio_sample_rate: 48000,
            audio_channels: "stereo".to_string(),
            audio_use_limiter: true,
            audio_layers: 32,
            audio_min_velocity: 1,
            export_format: "MP4".to_string(),
            encoder: "H.264".to_string(),
            encoder_backend: "Software (CPU)".to_string(),
            export_path: None,
        }
    }
}

impl Config {
    /// Path to the config file in the user's config directory.
    pub fn path() -> PathBuf {
        let mut path = if cfg!(target_os = "macos") {
            dirs_data_dir()
        } else if cfg!(target_os = "linux") {
            dirs_config_dir()
        } else if cfg!(target_os = "windows") {
            dirs_data_dir()
        } else {
            std::env::current_dir().unwrap_or_default()
        };
        path.push("nezha");
        std::fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    /// Load config from disk, or return defaults.
    pub fn load() -> Self {
        let path = Self::path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save config to disk.
    pub fn save(&self) {
        let path = Self::path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            std::fs::write(&path, json).ok();
        }
    }

    /// Apply the config to the app state.
    pub fn apply(&self, ui: &mut crate::app::UiState, project: &mut crate::app::ProjectState) {
        // Theme
        ui.theme_mode = match self.theme_mode.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            "system" => ThemeMode::System,
            _ => ThemeMode::Dark,
        };

        // SoundFonts
        project.soundfonts = self
            .soundfont_paths
            .iter()
            .map(|p| SoundFontEntry { path: p.clone() })
            .collect();

        // Audio device
        ui.audio_device_name = self.audio_device_name.clone();

        // Render settings
        project.render.width = self.render_width;
        project.render.height = self.render_height;
        project.render.fps = self.fps;
        project.render.audio_sample_rate = self.audio_sample_rate;
        project.render.audio_channels = match self.audio_channels.as_str() {
            "mono" => nezha_xsynth::ChannelCount::Mono,
            _ => nezha_xsynth::ChannelCount::Stereo,
        };
        project.render.audio_use_limiter = self.audio_use_limiter;
        project.render.audio_layers = self.audio_layers;
        project.render.audio_min_velocity = self.audio_min_velocity;

        // Export settings
        ui.export_format = self.export_format.clone();
        ui.encoder = self.encoder.clone();
        ui.encoder_backend = self.encoder_backend.clone();
        ui.export_path = self.export_path.clone();
    }

    /// Read current app state into a Config for saving.
    pub fn from_ui(ui: &crate::app::UiState, project: &crate::app::ProjectState) -> Self {
        Self {
            soundfont_paths: project
                .soundfonts
                .iter()
                .map(|sf| sf.path.clone())
                .collect(),
            audio_device_name: ui.audio_device_name.clone(),
            theme_mode: match ui.theme_mode {
                ThemeMode::Dark => "dark".to_string(),
                ThemeMode::Light => "light".to_string(),
                ThemeMode::System => "system".to_string(),
            },
            render_width: project.render.width,
            render_height: project.render.height,
            fps: project.render.fps,
            audio_sample_rate: project.render.audio_sample_rate,
            audio_channels: match project.render.audio_channels {
                nezha_xsynth::ChannelCount::Stereo => "stereo".to_string(),
                nezha_xsynth::ChannelCount::Mono => "mono".to_string(),
            },
            audio_use_limiter: project.render.audio_use_limiter,
            audio_layers: project.render.audio_layers,
            audio_min_velocity: project.render.audio_min_velocity,
            export_format: ui.export_format.clone(),
            encoder: ui.encoder.clone(),
            encoder_backend: ui.encoder_backend.clone(),
            export_path: ui.export_path.clone(),
        }
    }
}

fn dirs_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Users\\Default\\AppData\\Roaming"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        std::env::current_dir().unwrap_or_default()
    }
}

fn dirs_config_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        dirs_data_dir()
    }
}
