# 0021 — Render loop and MP4 output

- **Milestone:** M1 (first end-to-end video)
- **Depends on:** 0011, 0012, 0013, 0014, 0015, 0016, 0019, 0020
- **Files:** `src/app.rs`, `src/lib.rs`, `src/main.rs`, `tests/e2e_smoke.rs`
- **Feature:** `projectm`

## Goal

Wire everything together:

1. parse and validate the CLI,
2. decode/prepare the audio,
3. compute the frame count,
4. scan/shuffle presets,
5. create the renderer and ffmpeg,
6. loop over frames: switch preset if due, feed audio, render, write frame,
7. finalize ffmpeg.

This is the first ticket that produces a real MP4.

## Rules

- KISS: one `run_with_config` function, one loop. No worker threads.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below.

## Implementation

### 1. Create `src/app.rs`

```rust
//! Application orchestration.

use std::io::IsTerminal;

use anyhow::{ensure, Context, Result};
use clap::Parser;

use crate::audio;
use crate::cli::Cli;
use crate::config::RenderConfig;
use crate::encoder::Encoder;
use crate::presets::{self, PresetSchedule};
use crate::progress::Progress;
use crate::renderer::Renderer;
use crate::timebase::{frame_time_seconds, total_frames};

/// Parse the CLI and run a render.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = RenderConfig::from_cli(cli)?;
    run_with_config(config)
}

/// Run a render with an already-validated configuration.
///
/// This is public so the ignored end-to-end test can call it without
/// synthesizing command-line arguments.
pub fn run_with_config(config: RenderConfig) -> Result<()> {
    eprintln!("seed: {}", config.seed);

    let audio = audio::prepare(&config.input).context("preparing audio")?;
    let frame_count = total_frames(audio.frames(), config.fps);
    ensure!(
        frame_count > 0,
        "audio is too short to render at {} fps",
        config.fps
    );

    let preset_files = presets::scan(&config.presets)?;
    let preset_files = presets::shuffle(preset_files, config.seed);
    let mut schedule = PresetSchedule::new(
        preset_files,
        config.preset_duration,
        config.fps,
    )?;

    let mut renderer = Renderer::new(&config, schedule.current())
        .context("initializing the renderer")?;
    let mut encoder = Encoder::spawn(&config.encoder_config())
        .context("starting ffmpeg")?;

    let mut frame = vec![0_u8; config.frame_bytes()?];

    let progress = if std::io::stderr().is_terminal() {
        Progress::new(frame_count, config.fps)
    } else {
        Progress::hidden()
    };

    for frame_index in 0..frame_count {
        if let Some(preset) = schedule.advance(frame_index) {
            renderer.load_preset(preset, true)?;
        }

        renderer.feed_audio(frame_index, &audio.samples)?;

        let time = frame_time_seconds(frame_index, config.fps);
        renderer.render_frame(time, &mut frame)?;
        encoder.write_frame(&frame)?;

        progress.set_frame(frame_index + 1);
    }

    encoder.finish()?;
    progress.finish();

    eprintln!("wrote {}", config.output.display());
    Ok(())
}
```

### 2. Register the module in `src/lib.rs`

```rust
#[cfg(feature = "projectm")]
pub mod app;
```

### 3. Replace `src/main.rs`

```rust
fn main() {
    if let Err(err) = pmvid::app::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
```

The binary is already gated by `required-features = ["projectm"]` in
`Cargo.toml`.

## Tests

### Ignored end-to-end test: `tests/e2e_smoke.rs`

```rust
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
```

## Tests to run

Default suite (no native libraries):

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Native build and ignored e2e:

```sh
cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture
```

If EGL surfaceless fails, prefix with `xvfb-run -a`.

Manual render:

```sh
cargo run --release --features projectm -- \
  -i /path/to/song.mp3 -o /tmp/song.mp4 --presets tests/fixtures
ffprobe -v error -show_entries stream=codec_name,width,height -of default=nw=1 /tmp/song.mp4
```

## Acceptance criteria

- [x] `cargo run --features projectm -- ...` produces an MP4.
- [x] The MP4 has an H.264 video stream and an AAC audio stream.
- [x] The output duration is approximately the input duration.
- [x] Audio and video stay in sync on a short clip.
- [x] The seed is printed so the render can be reproduced.
- [x] A zero-frame audio file returns a clear error.
- [x] The ignored e2e test passes manually.

## Notes for the implementer

- The order inside the loop is deliberate: switch preset, feed audio, render,
  write. Do not feed the audio twice.
- `frame_time_seconds(frame_index, fps)` starts at 0.0. projectM recommends
  setting frame time to 0.0 on the first frame.
- Do not flip the frame in Rust. `EncoderConfig::args` adds `-vf vflip`.
- Do not use `frame_index * samples_per_frame`. Always use
  `feed_audio`/`frame_sample_range`.
- If the output is shorter than the audio, check that `total_frames` uses
  `audio.frames()` (samples per channel), not `audio.samples.len()`.
- If ffmpeg reports "height not divisible by 2", check CLI validation; the
  e2e test uses 64×64.
