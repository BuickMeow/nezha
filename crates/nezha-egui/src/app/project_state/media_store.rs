use nezha_media::{DecodedAudio, DecodedFrame, MediaInfo, MediaType, StreamingDecoder};
use std::collections::HashMap;

pub struct MediaEntry {
    pub info: MediaInfo,
    pub image_rgba: Option<Vec<u8>>,
    pub image_width: u32,
    pub image_height: u32,
}

pub struct MediaStore {
    pub entries: Vec<MediaEntry>,
    pub video_decoders: HashMap<usize, StreamingDecoder>,
}

impl Default for MediaStore {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            video_decoders: HashMap::new(),
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
        let decoder = StreamingDecoder::new(&info.path, info.width, info.height, info.fps);
        self.video_decoders.insert(idx, decoder);
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

    pub fn get_video_frame(&mut self, media_idx: usize, time_secs: f64) -> Option<&DecodedFrame> {
        let decoder = self.video_decoders.get_mut(&media_idx)?;
        decoder.get_frame(time_secs)
    }

    pub fn decode_audio(&self, media_idx: usize, target_sample_rate: u32) -> Option<DecodedAudio> {
        let entry = self.entries.get(media_idx)?;
        if entry.info.media_type != MediaType::Audio && !entry.info.has_audio {
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
