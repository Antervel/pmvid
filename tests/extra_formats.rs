//! Integration tests for extra supported audio formats.

use std::path::Path;
use std::process::Command;

use pmvid::audio::prepare;

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
}

fn generate(path: &Path, codec: &str) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.25",
            "-c:a",
            codec,
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "ffmpeg failed to create {}",
        path.display()
    );
}

fn assert_prepared(path: &Path) {
    let audio = prepare(path).unwrap();
    assert_eq!(audio.sample_rate, 44_100);
    assert_eq!(audio.channels, 2);
    assert!(
        (9_000..=14_000).contains(&audio.frames()),
        "unexpected frame count for {}: {}",
        path.display(),
        audio.frames()
    );
}

#[test]
fn flac_is_supported() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.flac");
    generate(&path, "flac");
    assert_prepared(&path);
}

#[test]
fn ogg_vorbis_is_supported() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.ogg");
    generate(&path, "libvorbis");
    assert_prepared(&path);
}

#[test]
fn wav_is_supported() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    generate(&path, "pcm_s16le");
    assert_prepared(&path);
}

#[test]
fn aac_in_m4a_is_supported() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.m4a");
    generate(&path, "aac");
    assert_prepared(&path);
}
