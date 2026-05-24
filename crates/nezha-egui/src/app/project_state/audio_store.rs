use nezha_xsynth::Limiter;

/// A rendered audio entry — stores PCM samples in memory.
#[derive(Clone, Debug)]
pub struct AudioEntry {
    /// Display name (derived from MIDI file name or user-specified).
    #[allow(dead_code)]
    pub name: String,
    /// Sample rate in Hz (e.g. 48000).
    #[allow(dead_code)]
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Interleaved 32-bit float PCM samples, range [-1.0, 1.0].
    pub samples: Vec<f32>,
    /// The MIDI store index this audio was rendered from.
    pub midi_idx: usize,
}

impl AudioEntry {
    /// Total number of frames (one frame = one sample per channel).
    pub fn frame_count(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    /// Get the frame index at a given time (seconds), clamped.
    #[allow(dead_code)]
    pub fn frame_at_time(&self, time_sec: f64) -> usize {
        let idx = (time_sec * self.sample_rate as f64) as usize;
        idx.min(self.frame_count().saturating_sub(1))
    }

    /// Get the sample index (interleaved) at a given time.
    #[expect(dead_code)]
    pub fn sample_index_at_time(&self, time_sec: f64) -> usize {
        let ch = self.channels as usize;
        self.frame_at_time(time_sec) * ch
    }
}

/// Collection of all rendered audio in the project.
#[derive(Clone, Default)]
pub struct AudioStore {
    pub entries: Vec<AudioEntry>,
}

impl AudioStore {
    pub fn insert(&mut self, entry: AudioEntry) -> usize {
        let idx = self.entries.len();
        self.entries.push(entry);
        idx
    }

    #[expect(dead_code)]
    pub fn remove(&mut self, idx: usize) {
        if idx < self.entries.len() {
            self.entries.remove(idx);
        }
    }

    #[expect(dead_code)]
    pub fn get(&self, idx: usize) -> Option<&AudioEntry> {
        self.entries.get(idx)
    }

    #[expect(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[expect(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Mix all audio entries into a single stereo buffer for the given duration.
    /// Each entry is placed at its clip's time position on the timeline.
    ///
    /// Returns (interleaved stereo f32 samples, sample_rate).
    pub fn mix_timeline(
        &self,
        timeline_clips: &[(usize, f32, f32)], // (audio_idx, clip_start, clip_end)
        sample_rate: u32,
        duration_secs: f64,
    ) -> Vec<f32> {
        let total_frames = (duration_secs * sample_rate as f64).ceil() as usize;
        let mut mix_buf = vec![0.0_f32; total_frames * 2]; // always mix to stereo

        for &(audio_idx, clip_start, clip_end) in timeline_clips {
            let Some(entry) = self.entries.get(audio_idx) else {
                continue;
            };
            let clip_start_frame = (clip_start as f64 * sample_rate as f64) as usize;
            let clip_duration = (clip_end - clip_start) as f64;
            let entry_frames = entry.frame_count();
            let entry_duration = entry.duration_secs;
            let copy_frames = ((clip_duration.min(entry_duration) * sample_rate as f64) as usize)
                .min(total_frames.saturating_sub(clip_start_frame))
                .min(entry_frames);

            for f in 0..copy_frames {
                let mix_idx = (clip_start_frame + f) * 2;
                let src_idx = f * entry.channels as usize;
                if entry.channels == 2 {
                    // stereo: directly add
                    mix_buf[mix_idx] += entry.samples[src_idx];
                    mix_buf[mix_idx + 1] += entry.samples[src_idx + 1];
                } else {
                    // mono: duplicate to both channels
                    let s = entry.samples[src_idx];
                    mix_buf[mix_idx] += s;
                    mix_buf[mix_idx + 1] += s;
                }
            }
        }

        mix_buf
    }

    /// Mix all audio entries into a single stereo buffer, then apply
    /// a lookahead brickwall limiter across the **full mix**.
    ///
    /// This is the preferred method for both preview and export, so the
    /// limiter sees the actual summed waveform and can properly catch
    /// peaks that exceed the safe range.
    ///
    /// Parameters are identical to [`mix_timeline`].
    pub fn mix_master(
        &self,
        timeline_clips: &[(usize, f32, f32)],
        sample_rate: u32,
        duration_secs: f64,
    ) -> Vec<f32> {
        let mut mix = self.mix_timeline(timeline_clips, sample_rate, duration_secs);

        // mix_timeline always outputs stereo
        let channels = 2;
        let mut limiter = Limiter::new(
            sample_rate as f32,
            channels,
            -1.0,  // threshold:  -1 dBFS
            -0.3,  // ceiling:    -0.3 dBFS
            2.0,   // lookahead:  2 ms
            0.5,   // attack:     0.5 ms
            100.0, // release:    100 ms
        );
        limiter.process(&mut mix);

        mix
    }
}
