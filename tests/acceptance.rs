#![cfg(feature = "projectm")]

use std::path::PathBuf;
use std::process::Command;

use pmvid::app::run_with_config;
use pmvid::config::RenderConfig;

fn make_mp3(path: &std::path::Path, seconds: u32) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=440:duration={seconds}"),
            "-q:a",
            "9",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore = "long-running acceptance test; requires EGL/GL, projectM, ffmpeg"]
fn two_second_render_has_matching_duration_and_codecs() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("tone.mp3");
    let output = dir.path().join("tone.mp4");
    make_mp3(&input, 2);

    let config = RenderConfig {
        input: input.clone(),
        output: output.clone(),
        width: 320,
        height: 180,
        fps: 30,
        presets: PathBuf::from("tests/fixtures"),
        preset_duration: 20.0,
        beat_sensitivity: 1.0,
        seed: 42,
        crf: 28,
        ffmpeg: PathBuf::from("ffmpeg"),
    };

    run_with_config(config).unwrap();

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_name,width,height",
            "-of",
            "default=nw=1",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(probe.status.success());
    let info = String::from_utf8_lossy(&probe.stdout);

    assert!(info.contains("codec_name=h264"), "{info}");
    assert!(info.contains("codec_name=aac"), "{info}");
    assert!(info.contains("width=320"), "{info}");
    assert!(info.contains("height=180"), "{info}");
}
