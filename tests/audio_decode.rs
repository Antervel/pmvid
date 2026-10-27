//! Integration tests for audio decoding.

use std::f32::consts::PI;
use std::fs;
use std::path::Path;
use std::process::Command;

use pmvid::audio::decode_file;

fn write_sine_wav(path: &Path, sample_rate: u32, channels: u16) {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    let frames = sample_rate; // one second
    for frame in 0..frames {
        let value = (2.0 * PI * 440.0 * frame as f32 / sample_rate as f32).sin();
        let sample = (value * 0.5 * i16::MAX as f32) as i16;
        for _ in 0..channels {
            writer.write_sample(sample).unwrap();
        }
    }
    writer.finalize().unwrap();
}

#[test]
fn decodes_a_wav_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    write_sine_wav(&path, 48_000, 2);

    let audio = decode_file(&path).unwrap();

    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.frames(), 48_000);
    assert_eq!(audio.samples.len(), 48_000 * 2);
    assert!(audio.samples.iter().any(|s| s.abs() > 0.01));
}

#[test]
fn decodes_a_mono_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mono.wav");
    write_sine_wav(&path, 44_100, 1);

    let audio = decode_file(&path).unwrap();

    assert_eq!(audio.channels, 1);
    assert_eq!(audio.frames(), 44_100);
}

#[test]
fn rejects_a_corrupt_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.mp3");
    fs::write(&path, b"this is not an mp3 file").unwrap();

    let err = decode_file(&path).unwrap_err();
    let message = format!("{err:#}");
    assert!(
        message.contains("unsupported or corrupt audio file"),
        "unexpected error: {message}"
    );
}

#[test]
fn rejects_a_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.mp3");

    let err = decode_file(&path).unwrap_err();
    assert!(format!("{err:#}").contains("opening audio file"));
}

#[test]
fn skips_corrupt_packets_instead_of_failing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.mp3");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-c:a",
            "libmp3lame",
            "-b:a",
            "32k",
            path.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let mut bytes = fs::read(&path).unwrap();
    let sync = bytes
        .windows(2)
        .position(|w| w[0] == 0xFF && w[1] == 0xFB)
        .unwrap();
    bytes[sync + 10] ^= 0xFF;
    fs::write(&path, &bytes).unwrap();

    let audio = decode_file(&path).unwrap();

    assert_eq!(audio.sample_rate, 44_100);
    assert_eq!(audio.channels, 1);
    assert!(audio.frames() > 0);
}
