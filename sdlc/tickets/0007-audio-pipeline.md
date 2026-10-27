# 0007 — Audio pipeline and MP3/WAV tests

- **Milestone:** M1
- **Depends on:** 0004, 0005, 0006
- **Files:** `src/audio.rs`, `tests/audio_pipeline.rs`
- **Feature:** default

## Goal

Provide one function that the renderer can call:

```rust
let audio = audio::prepare(path)?;
// audio.samples is interleaved stereo f32 at 44.1 kHz
```

This ticket wires together decode → stereo → resample and proves the MP3 path
works.

## Rules

- KISS: one public entry point, no options.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Add to `src/audio.rs`:

```rust
use crate::SAMPLE_RATE;

/// Audio ready for projectM: interleaved stereo `f32` at 44.1 kHz.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
}

impl PreparedAudio {
    /// Number of sample frames (samples per channel).
    pub fn frames(&self) -> u64 {
        (self.samples.len() / self.channels) as u64
    }
}

/// Decode any supported file, convert it to stereo, and resample to 44.1 kHz.
pub fn prepare(path: &Path) -> Result<PreparedAudio> {
    let decoded = decode_file(path)?;
    let stereo = to_stereo(&decoded.samples, decoded.channels)?;
    let samples = resample_linear(&stereo, decoded.sample_rate, SAMPLE_RATE, 2)?;

    ensure!(
        !samples.is_empty(),
        "audio file '{}' produced no samples after resampling",
        path.display()
    );

    Ok(PreparedAudio {
        samples,
        sample_rate: SAMPLE_RATE,
        channels: 2,
    })
}
```

`PreparedAudio::frames()` divides by `channels` without a guard. The constructor
always sets `channels = 2`, so that is safe. Add a `debug_assert!(self.channels > 0)`
if you want extra safety, but do not make the method return `Result`.

## Tests

### `tests/audio_pipeline.rs`

```rust
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
```

The MP3 test skips only when ffmpeg is missing. ffmpeg is a documented system
dependency, so this is acceptable; the rest of the suite still passes.

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets audio
cargo test --all-targets
```

Manual check with a real file:

```sh
cargo test --all-targets -- --nocapture audio
```

## Acceptance criteria

- [x] `audio::prepare` is the only function the renderer needs for audio.
- [x] A 48 kHz stereo WAV becomes 44.1 kHz stereo with the expected frame count.
- [x] A mono WAV becomes duplicated stereo.
- [x] An MP3 generated by ffmpeg decodes to roughly 0.25 s of stereo audio.
- [x] Corrupt input returns a clear error.
- [x] No resampling or channel code is duplicated outside `audio.rs`.

## Notes for the implementer

- Keep `decode_file` public; ticket `0027` uses it for extra-format tests.
- If the MP3 frame count is outside the tolerance, first check the ffmpeg
  command manually:
  ```sh
  ffmpeg -y -f lavfi -i "sine=frequency=440:duration=0.25" -q:a 9 /tmp/tone.mp3
  ffprobe -v error -show_entries format=duration -of csv=p=0 /tmp/tone.mp3
  ```
  Then adjust the tolerance, not the decoder.
- Do not shell out to ffmpeg in production code for audio decoding. Symphonia
  does the decoding; ffmpeg is only used in this test to create a fixture.
