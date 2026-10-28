use std::fs;
use std::path::PathBuf;

use clap::Parser;
use pmvid::audio::prepare;
use pmvid::cli::Cli;
use pmvid::config::RenderConfig;
use pmvid::encoder::{Encoder, EncoderConfig};
use pmvid::presets::scan;

#[test]
fn missing_input_file_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "pmvid",
        "-i",
        dir.path().join("missing.mp3").to_str().unwrap(),
        "-o",
        dir.path().join("out.mp4").to_str().unwrap(),
    ])
    .unwrap();

    let err = RenderConfig::from_cli(cli).unwrap_err();
    assert!(format!("{err:#}").contains("input file"));
}

#[test]
fn corrupt_audio_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.mp3");
    fs::write(&path, b"not audio").unwrap();

    let err = prepare(&path).unwrap_err();
    assert!(format!("{err:#}").contains("unsupported or corrupt audio file"));
}

#[test]
fn empty_preset_directory_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();

    let err = scan(dir.path()).unwrap_err();
    assert!(format!("{err:#}").contains("no .milk presets found"));
}

#[test]
fn missing_ffmpeg_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = EncoderConfig {
        ffmpeg: dir.path().join("no-such-ffmpeg"),
        input: PathBuf::from("in.mp3"),
        output: dir.path().join("out.mp4"),
        width: 64,
        height: 64,
        fps: 30,
        crf: 20,
    };

    let err = Encoder::spawn(&cfg).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("failed to spawn ffmpeg"));
    assert!(message.contains("install ffmpeg"));
}
