//! Integration tests for the audio pipeline.

use std::f32::consts::PI;
use std::path::Path;
use std::process::Command;

use pmvid::audio::prepare;

fn write_sine_wav(path: &Path, sample_rate: u32, channels: u16, seconds: f32) {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    let frames = (sample_rate as f32 * seconds) as u32;
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
fn wav_is_resampled_to_stereo_44k() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    write_sine_wav(&path, 48_000, 2, 1.0);

    let audio = prepare(&path).unwrap();

    assert_eq!(audio.sample_rate, 44_100);
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.frames(), 44_100);
    assert_eq!(audio.samples.len(), 44_100 * 2);
}

#[test]
fn mono_wav_becomes_stereo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mono.wav");
    write_sine_wav(&path, 44_100, 1, 0.5);

    let audio = prepare(&path).unwrap();

    assert_eq!(audio.frames(), 22_050);
    assert_eq!(audio.samples.len(), 22_050 * 2);
    // The two channels should be identical for a mono source.
    for frame in audio.samples.chunks_exact(2) {
        assert!((frame[0] - frame[1]).abs() < 1e-6);
    }
}

#[test]
fn mp3_is_decoded_and_resampled_when_ffmpeg_is_available() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("skipping MP3 test: ffmpeg not found");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.mp3");

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.25",
            "-q:a",
            "9",
        ])
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg failed to create the MP3 fixture");

    let audio = prepare(&path).unwrap();

    assert_eq!(audio.sample_rate, 44_100);
    assert_eq!(audio.channels, 2);
    // MP3 adds encoder delay/padding, so allow a generous tolerance around
    // 0.25 s * 44,100 = 11,025 frames.
    assert!(
        (9_000..=14_000).contains(&audio.frames()),
        "unexpected frame count: {}",
        audio.frames()
    );
}

#[test]
fn corrupt_file_returns_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.mp3");
    std::fs::write(&path, b"not audio").unwrap();

    let err = prepare(&path).unwrap_err();
    assert!(format!("{err:#}").contains("unsupported or corrupt audio file"));
}
