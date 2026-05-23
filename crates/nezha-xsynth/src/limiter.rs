/// A lookahead brickwall limiter.
///
/// Implements:
/// - Lookahead delay (to catch transients before they hit)
/// - Peak detection with attack/release envelope
/// - Smooth gain reduction (no hard clipping)
/// - Configurable threshold and ceiling
pub struct Limiter {
    sample_rate: f32,
    channels: usize,
    lookahead_frames: usize,
    attack_coeff: f32,
    release_coeff: f32,
    threshold_linear: f32, // e.g. 0.85
    ceiling_linear: f32,   // e.g. 0.99

    // State
    delay_buf: Vec<f32>,
    delay_write: usize,
    envelope: f32,    // smoothed peak level (linear)
    gain: f32,        // current gain reduction (linear)
    makeup_gain: f32, // fixed makeup gain
}

impl Limiter {
    /// Create a new limiter.
    ///
    /// - `sample_rate`: audio sample rate in Hz
    /// - `channels`: number of audio channels
    /// - `threshold_db`: threshold in dB (e.g. -1.0 means start limiting at -1 dBFS)
    /// - `ceiling_db`: maximum output level in dB (e.g. -0.3)
    /// - `lookahead_ms`: lookahead in milliseconds (e.g. 2.0)
    /// - `attack_ms`: attack time in milliseconds (e.g. 0.5)
    /// - `release_ms`: release time in milliseconds (e.g. 100.0)
    pub fn new(
        sample_rate: f32,
        channels: usize,
        threshold_db: f32,
        ceiling_db: f32,
        lookahead_ms: f32,
        attack_ms: f32,
        release_ms: f32,
    ) -> Self {
        let lookahead_frames = ((lookahead_ms / 1000.0) * sample_rate).ceil() as usize;
        let attack_coeff = (-1.0 / (sample_rate * attack_ms / 1000.0)).exp();
        let release_coeff = (-1.0 / (sample_rate * release_ms / 1000.0)).exp();

        // Compensation: the gain reduction slightly lowers average level,
        // so apply a small fixed makeup.
        let makeup_gain = 1.0 + (threshold_db.abs() / 20.0) * 0.5;

        Self {
            sample_rate,
            channels,
            lookahead_frames,
            attack_coeff,
            release_coeff,
            threshold_linear: 10.0_f32.powf(threshold_db / 20.0),
            ceiling_linear: 10.0_f32.powf(ceiling_db / 20.0),
            delay_buf: vec![0.0; lookahead_frames * channels],
            delay_write: 0,
            envelope: 0.0,
            gain: 1.0,
            makeup_gain,
        }
    }

    /// Process interleaved PCM samples in-place.
    pub fn process(&mut self, samples: &mut [f32]) {
        let ch = self.channels;
        let delay_len = self.delay_buf.len();

        for frame in 0..samples.len() / ch {
            let frame_start = frame * ch;

            // 1. Compute peak level across channels for this frame
            let mut peak = 0.0_f32;
            for c in 0..ch {
                let s = samples[frame_start + c].abs();
                if s > peak {
                    peak = s;
                }
            }

            // 2. Envelope follower (peak detecting)
            if peak > self.envelope {
                // Attack
                self.envelope = peak + self.attack_coeff * (self.envelope - peak);
            } else {
                // Release
                self.envelope = peak + self.release_coeff * (self.envelope - peak);
            }

            // 3. Compute desired gain from envelope
            let desired_gain = if self.envelope > self.threshold_linear {
                // How much to reduce: bring the peak down to threshold
                let reduction = self.threshold_linear / self.envelope;
                // Then apply ceiling (brickwall)
                (reduction * self.ceiling_linear).min(self.ceiling_linear)
            } else {
                self.ceiling_linear
            };

            // 4. Smooth the gain (prevent clicks)
            // Use attack/release style smoothing on gain itself
            if desired_gain < self.gain {
                // Gain needs to drop (attack)
                self.gain = desired_gain + self.attack_coeff * (self.gain - desired_gain);
            } else {
                // Gain can rise (release)
                self.gain = desired_gain + self.release_coeff * (self.gain - desired_gain);
            }

            // 5. Apply gain to the LOOKAHEAD-delayed sample
            let read_idx = (self.delay_write + 1) % delay_len;
            let delayed_frame_start = (read_idx / ch) * ch;

            for c in 0..ch {
                let delayed_sample = self.delay_buf[read_idx + c];
                // Apply gain + makeup
                samples[frame_start + c] = delayed_sample * self.gain * self.makeup_gain;
            }

            // 6. Write current sample into delay buffer
            for c in 0..ch {
                self.delay_buf[self.delay_write + c] = samples[frame_start + c];
            }
            self.delay_write = (self.delay_write + ch) % delay_len;
        }
    }
}
