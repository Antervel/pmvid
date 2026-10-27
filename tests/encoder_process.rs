#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use pmvid::encoder::{Encoder, EncoderConfig};

fn fake_ffmpeg(dir: &Path, exit_code: i32) -> PathBuf {
    let path = dir.join("fake-ffmpeg.sh");
    let script = format!(
        r#"#!/bin/sh
out=""
for arg in "$@"; do out="$arg"; done
printf '%s\n' "$@" > "$out.args"
cat > "$out"
exit {exit_code}
"#
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn config(dir: &Path, ffmpeg: PathBuf, output: PathBuf) -> EncoderConfig {
    EncoderConfig {
        ffmpeg,
        input: dir.join("in.mp3"),
        output,
        width: 2,
        height: 2,
        fps: 1,
        crf: 20,
    }
}

#[test]
fn writes_frames_and_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(dir.path(), 0);
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), ffmpeg, output.clone());

    let mut encoder = Encoder::spawn(&cfg).unwrap();
    let frame = vec![0xABu8; cfg.frame_bytes().unwrap()];
    encoder.write_frame(&frame).unwrap();
    encoder.write_frame(&frame).unwrap();
    encoder.finish().unwrap();

    assert_eq!(
        fs::read(&output).unwrap().len(),
        2 * cfg.frame_bytes().unwrap()
    );

    let args = fs::read_to_string(output.with_extension("mp4.args")).unwrap();
    assert!(args.contains("-f\nrawvideo"));
    assert!(args.contains("-s\n2x2"));
    assert!(args.contains("-vf\nvflip"));
}

#[test]
fn rejects_a_frame_with_the_wrong_size() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(dir.path(), 0);
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), ffmpeg, output);
    let mut encoder = Encoder::spawn(&cfg).unwrap();

    let err = encoder.write_frame(&[0u8; 3]).unwrap_err();
    assert!(format!("{err:#}").contains("expected 16"));
}

#[test]
fn reports_a_non_zero_ffmpeg_exit() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(dir.path(), 3);
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), ffmpeg, output);

    let mut encoder = Encoder::spawn(&cfg).unwrap();
    let frame = vec![0xABu8; cfg.frame_bytes().unwrap()];
    encoder.write_frame(&frame).unwrap();

    let err = encoder.finish().unwrap_err();
    assert!(format!("{err:#}").contains("ffmpeg exited with"));
}

#[test]
fn reports_a_missing_ffmpeg_binary() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist-ffmpeg");
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), missing, output);

    let err = Encoder::spawn(&cfg).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("failed to spawn ffmpeg"));
    assert!(message.contains("install ffmpeg"));
}
