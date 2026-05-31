use crate::constants::MAX_PARALLEL_KEY_GROUPS;
use crate::keyboard;

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

/// Split the 128-key index range into fixed-size chunks for rayon parallel distribution.
///
/// With only 128 keys, rayon's work-stealing handles load balancing fine —
/// no need for weighted partitioning.
pub(crate) fn build_simple_key_chunks(render_keys: &[u8; 128]) -> Vec<std::ops::Range<usize>> {
    let n = render_keys.len();
    let num_chunks = rayon::current_num_threads()
        .min(MAX_PARALLEL_KEY_GROUPS)
        .max(1);
    let chunk_size = n.div_ceil(num_chunks);
    (0..n)
        .step_by(chunk_size)
        .map(|start| start..(start + chunk_size).min(n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::is_black_key;

    #[test]
    fn test_simple_key_chunks_covers_all_keys() {
        let render_keys: [u8; 128] = std::array::from_fn(|i| i as u8);
        let chunks = build_simple_key_chunks(&render_keys);
        let all_indices: Vec<usize> = chunks.iter().flat_map(|r| r.clone()).collect();
        assert_eq!(all_indices.len(), 128);
        for (i, &idx) in all_indices.iter().enumerate() {
            assert_eq!(idx, i);
        }
    }

    #[test]
    fn test_simple_key_chunks_non_empty() {
        let render_keys: [u8; 128] = std::array::from_fn(|i| i as u8);
        let chunks = build_simple_key_chunks(&render_keys);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(!chunk.is_empty());
        }
    }

    // ── build_render_key_order tests ──

    #[test]
    fn test_render_key_order_equal_width() {
        let order = build_render_key_order(true);
        assert_eq!(order.len(), 128);
        for i in 0..128u8 {
            assert_eq!(
                order[i as usize], i,
                "key {} should be at position {}",
                i, i
            );
        }
    }

    #[test]
    fn test_render_key_order_piano_whites_first() {
        let order = build_render_key_order(false);
        assert_eq!(order.len(), 128);

        // First 75 keys should all be white keys
        for (i, &key) in order.iter().enumerate().take(75) {
            assert!(
                !is_black_key(key),
                "expected white key at position {}, got key {}",
                i,
                key
            );
        }
        // Remaining 53 keys should all be black keys
        for (i, &key) in order.iter().enumerate().skip(75) {
            assert!(
                is_black_key(key),
                "expected black key at position {}, got key {}",
                i,
                key
            );
        }
    }

    #[test]
    fn test_render_key_order_piano_all_keys_present() {
        let order = build_render_key_order(false);
        let mut sorted = order;
        sorted.sort();
        for i in 0..128u8 {
            assert_eq!(sorted[i as usize], i, "key {} should be present", i);
        }
    }
}
