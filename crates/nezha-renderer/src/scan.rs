use rayon::prelude::*;

use crate::constants::SEEK_INDEX_BLOCK_SIZE;
use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::{RenderMode, RenderStyle};

/// Pre-computed prefix-max index for binary-searching note end times.
#[derive(Default, Clone)]
pub struct KeySeekIndex {
    block_prefix_max_end: Vec<f64>,
    block_prefix_max_end_tick: Vec<f64>,
}

impl KeySeekIndex {
    pub fn build(notes: &[nezha_types::Note]) -> Self {
        let n = notes.len();
        if n == 0 {
            return Self::default();
        }
        let blocks = n.div_ceil(SEEK_INDEX_BLOCK_SIZE);
        let mut max_end = Vec::with_capacity(blocks);
        let mut max_tick = Vec::with_capacity(blocks);
        for chunk in notes.chunks(SEEK_INDEX_BLOCK_SIZE) {
            max_end.push(
                chunk
                    .iter()
                    .map(|n| n.end)
                    .fold(f64::NEG_INFINITY, f64::max),
            );
            max_tick.push(
                chunk
                    .iter()
                    .map(|n| n.end_tick as f64)
                    .fold(f64::NEG_INFINITY, f64::max),
            );
        }
        Self {
            block_prefix_max_end: max_end,
            block_prefix_max_end_tick: max_tick,
        }
    }

    pub fn scan_index_for_time(&self, notes: &[nezha_types::Note], time: f64) -> usize {
        if self.block_prefix_max_end.is_empty() || notes.is_empty() {
            return 0;
        }
        let mut block = 0;
        while block < self.block_prefix_max_end.len() && self.block_prefix_max_end[block] <= time {
            block += 1;
        }
        let start = block.saturating_sub(1) * SEEK_INDEX_BLOCK_SIZE;
        let end = notes.len().min(start + SEEK_INDEX_BLOCK_SIZE);
        let mut scan = start;
        while scan < end && notes[scan].end <= time {
            scan += 1;
        }
        scan
    }

    pub fn scan_index_for_tick(&self, notes: &[nezha_types::Note], tick: f64) -> usize {
        if self.block_prefix_max_end_tick.is_empty() || notes.is_empty() {
            return 0;
        }
        let mut block = 0;
        while block < self.block_prefix_max_end_tick.len()
            && self.block_prefix_max_end_tick[block] <= tick
        {
            block += 1;
        }
        let start = block.saturating_sub(1) * SEEK_INDEX_BLOCK_SIZE;
        let end = notes.len().min(start + SEEK_INDEX_BLOCK_SIZE);
        let mut scan = start;
        while scan < end && (notes[scan].end_tick as f64) <= tick {
            scan += 1;
        }
        scan
    }
}

/// Per-key seek indices, built from the full MIDI source.
#[derive(Clone)]
pub struct NoteSeekIndex {
    pub per_key: [KeySeekIndex; 128],
}

impl NoteSeekIndex {
    pub fn build(source: &dyn NoteSource) -> Self {
        let mut per_key: [KeySeekIndex; 128] = std::array::from_fn(|_| KeySeekIndex::default());
        per_key.par_iter_mut().enumerate().for_each(|(key, idx)| {
            *idx = KeySeekIndex::build(source.key_notes(key as u8));
        });
        Self { per_key }
    }
}

/// Advance scan indices for all 128 keys past notes that have already ended.
#[allow(clippy::type_complexity)]
pub(crate) fn advance_scan_indices(
    midi: &dyn NoteSource,
    state: &mut MidiRenderState,
    time: f64,
    scroll_tick: f64,
    mode: RenderMode,
    seek_index: Option<&NoteSeekIndex>,
) {
    let (threshold, last_field, scan_with_seek, scan_linear): (
        f64,
        &mut f64,
        fn(&KeySeekIndex, &[nezha_types::Note], f64) -> usize,
        fn(&nezha_types::Note) -> f64,
    ) = match mode {
        RenderMode::TimeBased => (
            time,
            &mut state.last_time,
            KeySeekIndex::scan_index_for_time,
            |n| n.end,
        ),
        RenderMode::TickBased => (
            scroll_tick,
            &mut state.last_scroll_tick,
            KeySeekIndex::scan_index_for_tick,
            |n| n.end_tick as f64,
        ),
    };

    let rewound = threshold < *last_field;
    *last_field = threshold;

    if let Some(seek_index) = seek_index {
        state
            .scan_indices
            .par_iter_mut()
            .enumerate()
            .for_each(|(key, scan_slot)| {
                let notes = midi.key_notes(key as u8);
                *scan_slot = scan_with_seek(&seek_index.per_key[key], notes, threshold);
            });
    } else {
        if rewound {
            state.scan_indices = [0; 128];
        }
        state
            .scan_indices
            .par_iter_mut()
            .enumerate()
            .for_each(|(key, scan_slot)| {
                let notes = midi.key_notes(key as u8);
                if notes.is_empty() {
                    *scan_slot = 0;
                    return;
                }

                let mut scan = (*scan_slot).min(notes.len());
                while scan < notes.len() && scan_linear(&notes[scan]) <= threshold {
                    scan += 1;
                }
                *scan_slot = scan;
            });
    }
}

/// Compute the scroll tick for the current mode.
pub(crate) fn scroll_tick_for_mode(midi: &dyn NoteSource, time: f64, style: &RenderStyle) -> f64 {
    match style.render_mode {
        RenderMode::TimeBased => -1.0,
        RenderMode::TickBased => {
            let ticks_per_beat = midi.ticks_per_beat().unwrap_or(480) as f64;
            midi.tick_at_time(time)
                .unwrap_or(time * ticks_per_beat * 2.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_note(end: f64, end_tick: u32) -> nezha_types::Note {
        nezha_types::Note {
            key: 60,
            start: 0.0,
            end,
            start_tick: 0,
            end_tick,
            velocity: 100,
            channel: 0,
            track: 0,
        }
    }

    #[test]
    fn test_key_seek_index_empty() {
        let idx = KeySeekIndex::build(&[]);
        assert_eq!(idx.scan_index_for_time(&[], 10.0), 0);
        assert_eq!(idx.scan_index_for_tick(&[], 100.0), 0);
    }

    #[test]
    fn test_key_seek_index_single_block() {
        let notes: Vec<_> = (0..10)
            .map(|i| make_note(i as f64 + 1.0, i * 100 + 100))
            .collect();
        let idx = KeySeekIndex::build(&notes);

        assert_eq!(idx.scan_index_for_time(&notes, 5.5), 5);
        assert_eq!(idx.scan_index_for_time(&notes, 11.0), notes.len());

        assert_eq!(idx.scan_index_for_tick(&notes, 550.0), 5);
        assert_eq!(idx.scan_index_for_tick(&notes, 1100.0), notes.len());
    }

    #[test]
    fn test_key_seek_index_multi_block() {
        let count = 300usize;
        let notes: Vec<_> = (0..count)
            .map(|i| make_note(i as f64 * 0.5 + 0.5, (i as u32) * 50 + 50))
            .collect();
        let idx = KeySeekIndex::build(&notes);

        assert_eq!(idx.scan_index_for_time(&notes, 75.0), 150);
        assert_eq!(idx.scan_index_for_time(&notes, 150.0), 300);
        assert_eq!(idx.scan_index_for_tick(&notes, 7500.0), 150);
    }
}
