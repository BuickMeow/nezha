use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::MediaError;

pub fn ffmpeg_path() -> Result<PathBuf, MediaError> {
    let exe_name = "ffmpeg";

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let bundled = dir.join(exe_name);
        if bundled.is_file() {
            return Ok(bundled);
        }
    }

    if Command::new(exe_name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        return Ok(PathBuf::from(exe_name));
    }

    Err(MediaError::FfmpegNotFound)
}
