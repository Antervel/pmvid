# 0016 — Render config and seed resolution

- **Milestone:** M1
- **Depends on:** 0011, 0013, 0014
- **Files:** `src/config.rs`, `src/lib.rs`
- **Feature:** default

## Goal

Turn a parsed `Cli` into a validated `RenderConfig` that the app can use. This
is where filesystem checks and default-seed generation belong.

## Rules

- KISS: one struct, one constructor, one conversion to `EncoderConfig`.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Create `src/config.rs`:

```rust
//! Validated runtime configuration.

use std::path::PathBuf;

use anyhow::{bail, ensure, Result};

use crate::cli::Cli;
use crate::encoder::EncoderConfig;

/// Everything the renderer needs after CLI parsing and validation.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub presets: PathBuf,
    pub preset_duration: f64,
    pub beat_sensitivity: f32,
    pub seed: u64,
    pub crf: u8,
    pub ffmpeg: PathBuf,
}

impl RenderConfig {
    /// Validate the CLI and resolve paths and the RNG seed.
    pub fn from_cli(cli: Cli) -> Result<Self> {
        cli.validate()?;

        ensure!(
            cli.input.is_file(),
            "input file '{}' does not exist or is not a file",
            cli.input.display()
        );

        if let Some(parent) = cli.output.parent() {
            if !parent.as_os_str().is_empty() && !parent.is_dir() {
                bail!(
                    "output directory '{}' does not exist; create it first",
                    parent.display()
                );
            }
        }

        ensure!(
            cli.input != cli.output,
            "input and output must be different files"
        );

        let seed = cli.seed.unwrap_or_else(rand::random);

        Ok(Self {
            input: cli.input,
            output: cli.output,
            width: cli.width,
            height: cli.height,
            fps: cli.fps,
            presets: cli.presets,
            preset_duration: cli.preset_duration,
            beat_sensitivity: cli.beat_sensitivity,
            seed,
            crf: cli.crf,
            ffmpeg: cli.ffmpeg,
        })
    }

    /// Build the ffmpeg configuration for this render.
    pub fn encoder_config(&self) -> EncoderConfig {
        EncoderConfig {
            ffmpeg: self.ffmpeg.clone(),
            input: self.input.clone(),
            output: self.output.clone(),
            width: self.width,
            height: self.height,
            fps: self.fps,
            crf: self.crf,
        }
    }

    /// Number of bytes in one RGBA frame.
    pub fn frame_bytes(&self) -> Result<usize> {
        self.encoder_config().frame_bytes()
    }
}
```

Register the module:

```rust
pub mod config;
```

## Tests

Add to `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::fs;

    use clap::Parser;

    use super::*;

    fn cli_with_inputs(input: PathBuf, output: PathBuf) -> Cli {
        Cli::try_parse_from([
            "pmvid",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .unwrap()
    }

    #[test]
    fn resolves_defaults_and_a_provided_seed() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("song.mp3");
        fs::write(&input, b"placeholder").unwrap();
        let output = dir.path().join("song.mp4");

        let cli = Cli::try_parse_from([
            "pmvid",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--seed",
            "42",
        ])
        .unwrap();

        let config = RenderConfig::from_cli(cli).unwrap();

        assert_eq!(config.width, 1280);
        assert_eq!(config.height, 720);
        assert_eq!(config.fps, 30);
        assert_eq!(config.seed, 42);
        assert_eq!(config.preset_duration, 20.0);
        assert_eq!(config.beat_sensitivity, 1.0);
        assert_eq!(config.crf, 20);
        assert_eq!(config.ffmpeg, PathBuf::from("ffmpeg"));
    }

    #[test]
    fn generates_a_seed_when_none_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("song.mp3");
        fs::write(&input, b"placeholder").unwrap();
        let output = dir.path().join("song.mp4");

        let config = RenderConfig::from_cli(cli_with_inputs(input, output)).unwrap();
        // The value is random; the important thing is that a seed exists and
        // the config was created.
        let _ = config.seed;
    }

    #[test]
    fn rejects_a_missing_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("missing.mp3");
        let output = dir.path().join("song.mp4");

        let err = RenderConfig::from_cli(cli_with_inputs(input, output)).unwrap_err();
        assert!(format!("{err:#}").contains("does not exist or is not a file"));
    }

    #[test]
    fn rejects_a_missing_output_directory() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("song.mp3");
        fs::write(&input, b"placeholder").unwrap();
        let output = dir.path().join("missing-dir/song.mp4");

        let err = RenderConfig::from_cli(cli_with_inputs(input, output)).unwrap_err();
        assert!(format!("{err:#}").contains("output directory"));
    }

    #[test]
    fn rejects_identical_input_and_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("same.mp3");
        fs::write(&path, b"placeholder").unwrap();

        let err = RenderConfig::from_cli(cli_with_inputs(path.clone(), path)).unwrap_err();
        assert!(format!("{err:#}").contains("must be different"));
    }

    #[test]
    fn builds_matching_encoder_config() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("song.mp3");
        fs::write(&input, b"placeholder").unwrap();
        let output = dir.path().join("song.mp4");

        let cli = Cli::try_parse_from([
            "pmvid",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "-W",
            "640",
            "-H",
            "360",
            "-f",
            "24",
            "--crf",
            "18",
        ])
        .unwrap();

        let config = RenderConfig::from_cli(cli).unwrap();
        let encoder = config.encoder_config();

        assert_eq!(encoder.width, 640);
        assert_eq!(encoder.height, 360);
        assert_eq!(encoder.fps, 24);
        assert_eq!(encoder.crf, 18);
        assert_eq!(config.frame_bytes().unwrap(), 640 * 360 * 4);
    }
}
```

`Cli` is in scope through `use super::*`, but `Cli::try_parse_from` is a trait
method, so the tests also need `use clap::Parser;`.

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets config
cargo test --all-targets
```

## Acceptance criteria

- [x] `RenderConfig::from_cli` rejects a missing input file.
- [x] It rejects a non-existent output directory.
- [x] It rejects identical input and output paths.
- [x] A provided `--seed` is preserved exactly.
- [x] A missing `--seed` produces some `u64` seed.
- [x] `encoder_config()` carries width, height, fps, crf, paths, and ffmpeg.
- [x] `frame_bytes()` matches the encoder's frame size.

## Notes for the implementer

- Do not create the output directory automatically. Silent directory creation
  hides typos. The error tells the user what to do.
- The input file check uses `is_file()`, so directories are rejected too.
- The random seed is generated with `rand::random()`. The app should print it
  so a user can reproduce a render. Ticket `0021` adds that print.
- Do not canonicalize paths in this ticket. Relative paths are useful in error
  messages and tests.
