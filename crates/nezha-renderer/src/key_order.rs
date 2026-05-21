use crate::constants::MAX_PARALLEL_KEY_GROUPS;
use crate::keyboard;
use crate::source::NoteSource;

/// Build the order in which keys should be rendered.
///
/// When `equal_key_width` is true, keys are rendered in natural 0–127 order.
/// Otherwise, white keys are rendered first (bottom layer), then black keys on top.
pub(crate) fn build_render_key_order(equal_key_width: bool) -> [u8; 128] {
    let mut keys = [0u8; 128];
    if equal_key_width {
        for key in 0..128u8 {
            keys[key as usize] = key;
        }
    } else {
        let mut idx = 0usize;
        for key in 0..128u8 {
            if !keyboard::is_black_key(key) {
                keys[idx] = key;
                idx += 1;
            }
        }
        for key in 0..128u8 {
            if keyboard::is_black_key(key) {
                keys[idx] = key;
                idx += 1;
            }
        }
    }
    keys
}

/// Build parallel key groups for rayon work distribution.
///
/// Splits the 128-key index range into balanced chunks so that parallel
/// instance building is well-distributed across CPU threads.
pub(crate) fn build_parallel_key_groups(
    render_keys: &[u8; 128],
    scan_indices: &[usize; 128],
    midi: &dyn NoteSource,
) -> Vec<std::ops::Range<usize>> {
    let mut total_weight = 0usize;
    let mut active_key_count = 0usize;
    let mut weights = [0usize; 128];
    for (i, &key) in render_keys.iter().enumerate() {
        let notes = midi.key_notes(key);
        let remaining = notes.len().saturating_sub(scan_indices[key as usize]);
        let weight = remaining.max(1);
        weights[i] = weight;
        total_weight += weight;
        if !notes.is_empty() {
            active_key_count += 1;
        }
    }

    if active_key_count <= 1 {
        return vec![0..128];
    }

    let thread_budget = rayon::current_num_threads().max(1);
    let desired_groups = if total_weight < 8_192 {
        thread_budget
    } else {
        thread_budget.saturating_mul(2)
    }
    .min(MAX_PARALLEL_KEY_GROUPS)
    .min(active_key_count)
    .max(1);

    let target_weight = total_weight.div_ceil(desired_groups);
    let mut ranges = Vec::with_capacity(desired_groups);
    let mut start = 0usize;
    let mut acc = 0usize;

    for i in 0..128usize {
        let remaining_keys = 128usize - i;
        let remaining_groups = desired_groups.saturating_sub(ranges.len());
        if remaining_groups == 0 {
            break;
        }

        acc += weights[i];
        let should_split = acc >= target_weight && remaining_keys > remaining_groups;
        if should_split {
            ranges.push(start..(i + 1));
            start = i + 1;
            acc = 0;
        }
    }

    if start < 128 {
        ranges.push(start..128);
    }
    if ranges.is_empty() {
        ranges.push(0..128);
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SingleKeySource;
    impl NoteSource for SingleKeySource {
        fn key_notes(&self, key: u8) -> &[nezha_core::Note] {
            if key == 60 {
                static NOTES: [nezha_core::Note; 1] = [nezha_core::Note {
                    key: 60,
                    start: 0.0,
                    end: 10.0,
                    start_tick: 0,
                    end_tick: 480,
                    velocity: 100,
                    channel: 0,
                    track: 0,
                }];
                &NOTES
            } else {
                &[]
            }
        }
        fn duration(&self) -> f64 {
            10.0
        }
    }

    #[test]
    fn test_build_parallel_key_groups_single_active() {
        let scan_indices = [0usize; 128];
        let mut render_keys = [0u8; 128];
        for i in 0..128u8 {
            render_keys[i as usize] = i;
        }

        let groups = build_parallel_key_groups(&render_keys, &scan_indices, &SingleKeySource);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], 0..128);
    }
}
