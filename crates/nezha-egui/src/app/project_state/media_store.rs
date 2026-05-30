use nezha_media::{DecodedAudio, MediaInfo, MediaType, VideoDecoder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MediaEntry {
    pub info: MediaInfo,
    pub image_rgba: Option<Vec<u8>>,
    pub image_width: u32,
    pub image_height: u32,
}

pub struct MediaStore {
    pub entries: Vec<MediaEntry>,
    pub video_decoders: HashMap<usize, Arc<Mutex<VideoDecoder>>>,
    pub frame_cache: HashMap<usize, nezha_media::FrameCache>,
}

impl Default for MediaStore {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            video_decoders: HashMap::new(),
            frame_cache: HashMap::new(),
        }
    }
}

impl MediaStore {
    pub fn add_image(&mut self, info: MediaInfo, rgba: Vec<u8>, width: u32, height: u32) -> usize {
        let idx = self.entries.len();
        self.entries.push(MediaEntry {
            info,
            image_rgba: Some(rgba),
            image_width: width,
            image_height: height,
        });
        idx
    }

    pub fn add_video(&mut self, info: MediaInfo) -> usize {
        let idx = self.entries.len();
        let decoder = VideoDecoder::new(&info.path, info.width, info.height);
        self.video_decoders
            .insert(idx, Arc::new(Mutex::new(decoder)));
        self.frame_cache.insert(idx, nezha_media::FrameCache::new(64));
        self.entries.push(MediaEntry {
            info,
            image_rgba: None,
            image_width: 0,
            image_height: 0,
        });
        idx
    }

    pub fn add_audio(&mut self, info: MediaInfo) -> usize {
        let idx = self.entries.len();
        self.entries.push(MediaEntry {
            info,
            image_rgba: None,
            image_width: 0,
            image_height: 0,
        });
        idx
    }

    pub fn get(&self, idx: usize) -> Option<&MediaEntry> {
        self.entries.get(idx)
    }

    pub fn decode_video_frame(
        &mut self,
        media_idx: usize,
        time_secs: f64,
    ) -> Option<&nezha_media::DecodedFrame> {
        let cache = self.frame_cache.get_mut(&media_idx)?;
        let decoder = self.video_decoders.get(&media_idx)?;

        let key = (time_secs * 1000.0).round() as u64;

        if cache.get(key).is_some() {
            return cache.get(key);
        }

        let frame = {
            let dec = decoder.lock().ok()?;
            dec.decode_frame(time_secs).ok()?
        };

        cache.insert(key, frame);
        cache.get(key)
    }

    pub fn decode_audio(
        &self,
        media_idx: usize,
        target_sample_rate: u32,
    ) -> Option<DecodedAudio> {
        let entry = self.entries.get(media_idx)?;
        if entry.info.media_type != MediaType::Audio {
            return None;
        }
        nezha_media::AudioDecoder::decode_file(&entry.info.path, target_sample_rate).ok()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
