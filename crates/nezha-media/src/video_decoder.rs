use std::collections::VecDeque;
use std::io::Read;
use std::process::{Child, Command, Stdio};

use crate::ffmpeg_util::ffmpeg_path;

const MAX_BUFFER_FRAMES: usize = 64;

pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct StreamingDecoder {
    path: String,
    width: u32,
    height: u32,
    frame_size: usize,
    frame_duration: f64,

    child: Option<Child>,
    stdout: Option<std::io::BufReader<std::process::ChildStdout>>,

    buffer: VecDeque<DecodedFrame>,
    buffer_end_video_time: f64,

    eof: bool,
}

impl StreamingDecoder {
    pub fn new(path: &str, width: u32, height: u32, video_fps: f64) -> Self {
        let video_fps = if video_fps > 0.0 { video_fps } else { 30.0 };
        let frame_size = (width as usize) * (height as usize) * 4;
        Self {
            path: path.to_string(),
            width,
            height,
            frame_size,
            frame_duration: 1.0 / video_fps,
            child: None,
            stdout: None,
            buffer: VecDeque::with_capacity(MAX_BUFFER_FRAMES),
            buffer_end_video_time: 0.0,
            eof: false,
        }
    }

    pub fn get_frame(&mut self, time_secs: f64) -> Option<&DecodedFrame> {
        let time_secs = time_secs.max(0.0);

        if self.child.is_none() {
            self.spawn_ffmpeg(time_secs);
            return self.buffer.back();
        }

        let behind = self.buffer_end_video_time - time_secs;

        if behind < -self.frame_duration * 0.5 {
            self.spawn_ffmpeg(time_secs);
            return self.buffer.back();
        }

        if self.eof {
            return self.buffer.back();
        }

        if behind < self.frame_duration * 2.0 {
            self.read_one_frame();
        }

        self.buffer.back()
    }

    fn spawn_ffmpeg(&mut self, seek_secs: f64) {
        self.kill_child();
        self.buffer.clear();
        self.eof = false;

        let ffmpeg = match ffmpeg_path() {
            Ok(p) => p,
            Err(_) => {
                self.eof = true;
                return;
            }
        };

        let child = Command::new(&ffmpeg)
            .args([
                "-v",
                "quiet",
                "-ss",
                &format!("{:.4}", seek_secs),
                "-i",
                &self.path,
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match child {
            Ok(c) => c,
            Err(_) => {
                self.eof = true;
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                self.eof = true;
                return;
            }
        };

        self.child = Some(child);
        self.stdout = Some(std::io::BufReader::with_capacity(self.frame_size, stdout));
        self.buffer_end_video_time = seek_secs;

        self.read_one_frame();
    }

    fn read_one_frame(&mut self) -> bool {
        if self.eof {
            return false;
        }

        if self.buffer.len() >= MAX_BUFFER_FRAMES {
            self.buffer.pop_front();
        }

        let stdout = match &mut self.stdout {
            Some(s) => s,
            None => return false,
        };

        let mut data = vec![0u8; self.frame_size];
        let mut filled = 0usize;
        while filled < self.frame_size {
            match stdout.read(&mut data[filled..]) {
                Ok(0) => {
                    self.eof = true;
                    data.truncate(filled);
                    if filled > 0 {
                        self.buffer.push_back(DecodedFrame {
                            width: self.width,
                            height: self.height,
                            rgba: data,
                        });
                        self.buffer_end_video_time += self.frame_duration;
                    }
                    return filled > 0;
                }
                Ok(n) => filled += n,
                Err(_) => {
                    self.eof = true;
                    return false;
                }
            }
        }

        self.buffer.push_back(DecodedFrame {
            width: self.width,
            height: self.height,
            rgba: data,
        });
        self.buffer_end_video_time += self.frame_duration;
        true
    }

    fn kill_child(&mut self) {
        self.stdout = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for StreamingDecoder {
    fn drop(&mut self) {
        self.kill_child();
    }
}
