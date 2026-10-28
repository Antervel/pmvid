#![cfg(feature = "projectm")]

use std::path::PathBuf;
use std::process::Command;

use pmvid::app::run_with_config;
use pmvid::config::RenderConfig;

fn make_mp3(path: &std::path::Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1.0",
            "-q:a",
            "9",
        ])
        .arg(path)
        .status()
        .expect("ffmpeg must be installed for the e2e test");
    assert!(status.success());
}

#[test]
#[ignore = "requires EGL/GL, projectM, ffmpeg, and the native projectm feature"]
fn renders_a_short_mp4() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("tone.mp3");
    let output = dir.path().join("tone.mp4");
    make_mp3(&input);

    let config = RenderConfig {
        input,
        output: output.clone(),
        width: 64,
        height: 64,
        fps: 30,
        presets: PathBuf::from("tests/fixtures"),
        preset_duration: 20.0,
        beat_sensitivity: 1.0,
        seed: 42,
        crf: 28,
        ffmpeg: PathBuf::from("ffmpeg"),
    };

    run_with_config(config).expect("render failed");

    let metadata = std::fs::metadata(&output).unwrap();
    assert!(metadata.len() > 0, "output file is empty");

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "csv=p=0",
        ])
        .arg(&output)
        .output()
        .expect("ffprobe must be installed for the e2e test");
    assert!(probe.status.success());

    let codecs = String::from_utf8_lossy(&probe.stdout);
    assert!(codecs.contains("h264"), "missing h264 stream: {codecs}");
    assert!(codecs.contains("aac"), "missing aac stream: {codecs}");
}
