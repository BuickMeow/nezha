use super::App;
use crate::app::error::AppError;
use crate::app::project_state::AudioEntry;
use nezha_media::MediaType;

impl App {
    pub(super) fn import_media_by_type(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        dialog = dialog.add_filter(
            "所有媒体文件",
            &[
                "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "ts", "m4v", "mp3", "wav",
                "flac", "ogg", "aac", "m4a", "wma", "opus", "png", "jpg", "jpeg", "bmp", "webp",
                "tiff", "gif",
            ],
        );

        if let Some(path) = dialog.pick_file() {
            let path_str = path.to_string_lossy().to_string();
            match nezha_media::probe_media(&path_str) {
                Ok(info) => match info.media_type {
                    MediaType::Image => match nezha_media::load_image(&path_str) {
                        Ok(img) => {
                            self.project
                                .media
                                .add_image(info, img.rgba, img.width, img.height);
                        }
                        Err(e) => {
                            self.project.last_error = Some(AppError::MediaProbe(e));
                        }
                    },
                    MediaType::Video => {
                        self.project.media.add_video(info);
                    }
                    MediaType::Audio => {
                        self.project.media.add_audio(info);
                    }
                },
                Err(e) => {
                    self.project.last_error = Some(AppError::MediaProbe(e));
                }
            }
        }
    }

    pub(super) fn add_media_to_timeline(&mut self, media_idx: usize) {
        let entry = match self.project.media.get(media_idx) {
            Some(e) => e,
            None => return,
        };
        let info = &entry.info;
        let name = std::path::Path::new(&info.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("媒体")
            .to_string();

        match info.media_type {
            MediaType::Image => {
                let duration = self.project.duration() as f32;
                self.project
                    .timeline_state
                    .push_image_clip(media_idx, name, duration);
            }
            MediaType::Video => {
                let duration = info.duration_secs as f32;
                self.project
                    .timeline_state
                    .push_video_clip(media_idx, name.clone(), duration);
                if info.has_audio {
                    let sample_rate = self.project.render.audio_sample_rate;
                    if let Some(decoded) = self.project.media.decode_audio(media_idx, sample_rate) {
                        let audio_entry = AudioEntry {
                            name: name.clone(),
                            sample_rate: decoded.sample_rate,
                            channels: decoded.channels,
                            duration_secs: decoded.duration_secs,
                            samples: decoded.samples,
                            midi_idx: 0,
                        };
                        let audio_idx = self.project.audio.insert(audio_entry);
                        self.project.timeline_state.push_audio_clip(
                            audio_idx,
                            name,
                            info.duration_secs as f32,
                        );
                    }
                }
            }
            MediaType::Audio => {
                let sample_rate = self.project.render.audio_sample_rate;
                if let Some(decoded) = self.project.media.decode_audio(media_idx, sample_rate) {
                    let audio_entry = AudioEntry {
                        name: name.clone(),
                        sample_rate: decoded.sample_rate,
                        channels: decoded.channels,
                        duration_secs: decoded.duration_secs,
                        samples: decoded.samples,
                        midi_idx: 0,
                    };
                    let audio_idx = self.project.audio.insert(audio_entry);
                    self.project.timeline_state.push_audio_clip(
                        audio_idx,
                        name,
                        info.duration_secs as f32,
                    );
                } else {
                    self.project.last_error = Some(AppError::Other("音频解码失败".into()));
                }
            }
        }
    }

    pub(super) fn remove_media(&mut self, media_idx: usize) {
        if media_idx < self.project.media.len() {
            self.project.media.entries.remove(media_idx);
            self.project.media.video_decoders.remove(&media_idx);
        }
    }
}
