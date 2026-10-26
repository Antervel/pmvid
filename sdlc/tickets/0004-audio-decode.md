# 0004 — Decode audio to interleaved f32

- **Milestone:** M1
- **Depends on:** 0001
- **Files:** `src/audio.rs`, `src/lib.rs`, `tests/audio_decode.rs`
- **Feature:** default

## Goal

Decode an audio file with `symphonia` into interleaved `f32` samples, keeping
the file's native sample rate and channel count. Conversion to stereo and
44.1 kHz happens in tickets `0005` and `0006`.

This ticket only has to handle the common path: open the file, pick the default
audio track, decode packets, and concatenate the samples. Corrupt individual
packets should be skipped with a warning; a file that cannot be probed is an
error.

## Rules

- KISS: decode the whole file into memory. Streaming is not needed for v1.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

### 1. Create `src/audio.rs`

```rust
//! Audio decoding and conversion.

use std::fs::File;
use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded audio in the file's native sample rate and channel layout.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    /// Interleaved samples. Length is `frames * channels`.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
}

impl DecodedAudio {
    /// Number of sample frames (samples per channel).
    pub fn frames(&self) -> u64 {
        if self.channels == 0 {
            0
        } else {
            (self.samples.len() / self.channels) as u64
        }
    }
}

/// Decode an audio file to interleaved `f32` at its native sample rate.
pub fn decode_file(path: &Path) -> Result<DecodedAudio> {
    let file = File::open(path)
        .with_context(|| format!("opening audio file '{}'", path.display()))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| {
            format!(
                "unsupported or corrupt audio file '{}' (expected MP3, WAV, FLAC, or OGG)",
                path.display()
            )
        })?;

    let mut format = probed.format;

    let (track_id, codec_params) = {
        let track = format
            .default_track()
            .with_context(|| format!("no audio track found in '{}'", path.display()))?;
        (track.id, track.codec_params.clone())
    };

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .with_context(|| format!("unsupported audio codec in '{}'", path.display()))?;

    let mut samples = Vec::new();
    let mut sample_rate: Option<u32> = None;
    let mut channels: Option<usize> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                bail!("audio stream changed format mid-file; this is not supported");
            }
            Err(err) => {
                return Err(err).with_context(|| format!("reading audio from '{}'", path.display()));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();

                if let Some(rate) = sample_rate {
                    ensure!(
                        rate == spec.rate,
                        "sample rate changed from {rate} Hz to {} Hz mid-file",
                        spec.rate
                    );
                }
                if let Some(count) = channels {
                    ensure!(
                        count == spec.channels.count(),
                        "channel count changed mid-file"
                    );
                }

                sample_rate = Some(spec.rate);
                channels = Some(spec.channels.count());

                // Allocate per packet. This is simple and correct; optimize only
                // if profiling shows it matters.
                let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buffer.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buffer.samples());
            }
            Err(SymphoniaError::DecodeError(message)) => {
                eprintln!("warning: skipping corrupt audio frame: {message}");
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("decoding audio from '{}'", path.display()));
            }
        }
    }

    let sample_rate = sample_rate.context("audio stream has no sample rate")?;
    let channels = channels.context("audio stream has no channel layout")?;

    ensure!(
        channels > 0,
        "audio stream in '{}' has zero channels",
        path.display()
    );
    ensure!(
        !samples.is_empty(),
        "decoded zero audio samples from '{}'",
        path.display()
    );

    Ok(DecodedAudio {
        samples,
        sample_rate,
        channels,
    })
}
```

### 2. Register the module

```rust
pub mod audio;
```

## Tests

### `tests/audio_decode.rs`

```rust
use std::f32::consts::PI;
use std::fs;

use pmvid::audio::decode_file;

fn write_sine_wav(path: &std::path::Path, sample_rate: u32, channels: u16) {
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
```

`hound` is already a dev-dependency from ticket `0001`.

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets audio
cargo test --all-targets
```

## Acceptance criteria

- [ ] `decode_file` returns interleaved `f32` at the file's native rate.
- [ ] A 48 kHz stereo WAV decodes to 48,000 frames and 96,000 floats.
- [ ] A mono WAV reports `channels == 1`.
- [ ] A corrupt file returns an error containing `unsupported or corrupt audio file`.
- [ ] A missing file returns an error containing `opening audio file`.
- [ ] Decode errors from individual packets are warnings, not fatal.

## Notes for the implementer

- Do not resample or downmix here. That is tickets `0005` and `0006`.
- Do not use `format.default_track()` after taking a mutable borrow of
  `format`; copy `track.id` and `track.codec_params` first, as shown.
- If Symphonia's `SampleBuffer` API differs in the version Cargo resolves,
  read the compiler error and adjust the `SampleBuffer::new` call. Do not
  hand-roll sample-format conversion.
- MP3 decoding is covered indirectly: the `mp3` feature is enabled in
  `Cargo.toml`. Ticket `0007` adds an MP3 integration test that generates a
  file with ffmpeg.
