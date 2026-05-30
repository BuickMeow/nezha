use std::collections::HashMap;

use crate::video_decoder::DecodedFrame;

struct CacheEntry {
    frame: DecodedFrame,
    last_used: std::time::Instant,
}

pub struct FrameCache {
    cache: HashMap<u64, CacheEntry>,
    max_frames: usize,
}

impl FrameCache {
    pub fn new(max_frames: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_frames: max_frames.max(1),
        }
    }

    pub fn get(&mut self, key: u64) -> Option<&DecodedFrame> {
        if let Some(entry) = self.cache.get_mut(&key) {
            entry.last_used = std::time::Instant::now();
            Some(&entry.frame)
        } else {
            None
        }
    }

    pub fn insert(&mut self, key: u64, frame: DecodedFrame) {
        if self.cache.len() >= self.max_frames {
            self.evict_oldest();
        }
        self.cache.insert(
            key,
            CacheEntry {
                frame,
                last_used: std::time::Instant::now(),
            },
        );
    }

    pub fn get_or_decode(
        &mut self,
        key: u64,
        decode_fn: impl FnOnce() -> Result<DecodedFrame, crate::MediaError>,
    ) -> Result<&DecodedFrame, crate::MediaError> {
        if self.cache.contains_key(&key) {
            return self.get(key).ok_or_else(|| {
                crate::MediaError::InvalidFrame("cache entry disappeared".into())
            });
        }

        let frame = decode_fn()?;
        self.insert(key, frame);
        self.get(key).ok_or_else(|| {
            crate::MediaError::InvalidFrame("cache entry just inserted not found".into())
        })
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self
            .cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(k, _)| *k)
        {
            self.cache.remove(&oldest_key);
        }
    }
}

impl Default for FrameCache {
    fn default() -> Self {
        Self::new(128)
    }
}
