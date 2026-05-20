use std::env;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // OUT_DIR = .../target/{profile}/build/nezha-egui-*/out
    let target_profile_dir = out_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let platform_dir = match target.as_str() {
        t if t.contains("windows") => "windows-x86_64",
        t if t.contains("apple") && t.contains("aarch64") => "macos-aarch64",
        t if t.contains("apple") => "macos-x86_64",
        t if t.contains("linux") => "linux-x86_64",
        _ => {
            println!(
                "cargo:warning=Unsupported target for ffmpeg sidecar: {}",
                target
            );
            return;
        }
    };

    let ffmpeg_name = if target.contains("windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ffmpeg_src = manifest_dir
        .join("../../assets/ffmpeg")
        .join(platform_dir)
        .join(ffmpeg_name);

    if ffmpeg_src.exists() {
        let ffmpeg_dst = target_profile_dir.join(ffmpeg_name);
        if let Err(e) = std::fs::copy(&ffmpeg_src, &ffmpeg_dst) {
            println!(
                "cargo:warning=Failed to copy ffmpeg sidecar to {}: {}",
                ffmpeg_dst.display(),
                e
            );
        } else {
            println!("cargo:rerun-if-changed={}", ffmpeg_src.display());
        }
    } else {
        println!(
            "cargo:warning=FFmpeg sidecar not found at {}. Place the ffmpeg binary for your platform there before building the release.",
            ffmpeg_src.display()
        );
    }
}
