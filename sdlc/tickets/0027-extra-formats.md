# 0027 — Extra audio formats

- **Milestone:** M4
- **Depends on:** 0007
- **Files:** `Cargo.toml`, `tests/extra_formats.rs`, `README.md`
- **Feature:** default

## Goal

Support FLAC, WAV, OGG/Vorbis, and AAC/M4A in addition to MP3. Symphonia
already does the work; this ticket enables the codec/container features and
proves each format with a generated fixture.

## Rules

- KISS: no new decoder crate. Symphonia handles all of these.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

### 1. Update `Cargo.toml`

Change the Symphonia dependency to:

```toml
symphonia = { version = "0.5.5", features = ["mp3", "aac", "isomp4"] }
```

The default features already include WAV, FLAC, OGG, and Vorbis. `aac` adds the
AAC decoder; `isomp4` adds the MP4/M4A container. Do not enable `all`; it pulls
in codecs we do not need and slows the build.

### 2. Update the README

In the troubleshooting table and the feature list, say:

```markdown
Supported input formats: MP3, WAV, FLAC, OGG/Vorbis, AAC/M4A.
```

Do not claim formats that are not tested. If a format does not work, remove it
from the list and open a follow-up ticket.

## Tests

### `tests/extra_formats.rs`

```rust
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
    assert!(status.success(), "ffmpeg failed to create {}", path.display());
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
```

MP3 is already covered by `tests/audio_pipeline.rs`. If you want a single
table-driven test instead of four functions, refactor only after all four pass.

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets extra_formats
cargo test --all-targets
```

Manual check with a real file:

```sh
cargo test --all-targets -- --nocapture extra_formats
```

## Acceptance criteria

- [x] `symphonia` enables `mp3`, `aac`, and `isomp4`.
- [x] FLAC, OGG/Vorbis, WAV, and AAC/M4A each decode to stereo 44.1 kHz.
- [x] MP3 still passes its existing test.
- [x] The README lists exactly the supported formats.
- [x] No new decoder dependency was added.

## Notes for the implementer

- AAC decoding in Symphonia is less mature than MP3/FLAC. If the M4A test fails
  with a decode error, capture the ffmpeg command and the error, and open a
  follow-up ticket instead of hacking the decoder.
- The tests skip when ffmpeg is missing. Do not make them fail on machines
  without ffmpeg; it is a documented runtime dependency, but the pure-Rust
  suite should still run.
- Do not add a `--format` CLI flag. Symphonia probes the file extension and
  container headers.
