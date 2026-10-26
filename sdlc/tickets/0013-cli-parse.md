# 0013 — CLI parsing with clap

- **Milestone:** M1
- **Depends on:** 0001
- **Files:** `src/cli.rs`, `src/lib.rs`
- **Feature:** default

## Goal

Define the command-line interface from PRD section 7 with `clap` derive.
Parsing only; value validation is ticket `0014`.

## Rules

- KISS: one `Cli` struct, no subcommands yet.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Create `src/cli.rs`:

```rust
//! Command-line interface.

use std::path::PathBuf;

use clap::Parser;

/// Render a projectM visualization for an audio file into an MP4.
#[derive(Debug, Clone, Parser)]
#[command(name = "pmvid", version, about, long_about = None)]
pub struct Cli {
    /// Input audio file.
    #[arg(short = 'i', long, value_name = "FILE")]
    pub input: PathBuf,

    /// Output video file.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: PathBuf,

    /// Output width in pixels.
    #[arg(short = 'W', long, value_name = "PX", default_value_t = 1280)]
    pub width: u32,

    /// Output height in pixels.
    #[arg(short = 'H', long, value_name = "PX", default_value_t = 720)]
    pub height: u32,

    /// Output frames per second.
    #[arg(short = 'f', long, value_name = "N", default_value_t = 30)]
    pub fps: u32,

    /// Directory containing .milk presets.
    #[arg(short = 'p', long, value_name = "DIR", default_value = "./presets")]
    pub presets: PathBuf,

    /// Seconds to show each preset.
    #[arg(long, value_name = "SEC", default_value_t = 20.0)]
    pub preset_duration: f64,

    /// projectM beat sensitivity.
    #[arg(long, value_name = "F", default_value_t = 1.0)]
    pub beat_sensitivity: f32,

    /// RNG seed for the preset order. Omit for a random seed.
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,

    /// x264 CRF quality (lower is better, 0-51).
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub crf: u8,

    /// ffmpeg binary to use.
    #[arg(long, value_name = "PATH", default_value = "ffmpeg")]
    pub ffmpeg: PathBuf,
}
```

Register the module:

```rust
pub mod cli;
```

## Tests

Add to `src/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn clap_configuration_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_required_arguments_and_defaults() {
        let cli = parse(&["pmvid", "-i", "song.mp3", "-o", "song.mp4"]);

        assert_eq!(cli.input, PathBuf::from("song.mp3"));
        assert_eq!(cli.output, PathBuf::from("song.mp4"));
        assert_eq!(cli.width, 1280);
        assert_eq!(cli.height, 720);
        assert_eq!(cli.fps, 30);
        assert_eq!(cli.presets, PathBuf::from("./presets"));
        assert_eq!(cli.preset_duration, 20.0);
        assert_eq!(cli.beat_sensitivity, 1.0);
        assert_eq!(cli.seed, None);
        assert_eq!(cli.crf, 20);
        assert_eq!(cli.ffmpeg, PathBuf::from("ffmpeg"));
    }

    #[test]
    fn parses_short_flags() {
        let cli = parse(&[
            "pmvid", "-i", "in.mp3", "-o", "out.mp4", "-W", "640", "-H", "360", "-f", "24", "-p",
            "my-presets",
        ]);

        assert_eq!(cli.width, 640);
        assert_eq!(cli.height, 360);
        assert_eq!(cli.fps, 24);
        assert_eq!(cli.presets, PathBuf::from("my-presets"));
    }

    #[test]
    fn parses_long_options() {
        let cli = parse(&[
            "pmvid",
            "--input",
            "in.mp3",
            "--output",
            "out.mp4",
            "--width",
            "1920",
            "--height",
            "1080",
            "--fps",
            "60",
            "--presets",
            "/tmp/presets",
            "--preset-duration",
            "12.5",
            "--beat-sensitivity",
            "1.5",
            "--seed",
            "42",
            "--crf",
            "18",
            "--ffmpeg",
            "/usr/bin/ffmpeg",
        ]);

        assert_eq!(cli.width, 1920);
        assert_eq!(cli.height, 1080);
        assert_eq!(cli.fps, 60);
        assert_eq!(cli.presets, PathBuf::from("/tmp/presets"));
        assert_eq!(cli.preset_duration, 12.5);
        assert_eq!(cli.beat_sensitivity, 1.5);
        assert_eq!(cli.seed, Some(42));
        assert_eq!(cli.crf, 18);
        assert_eq!(cli.ffmpeg, PathBuf::from("/usr/bin/ffmpeg"));
    }

    #[test]
    fn input_and_output_are_required() {
        assert!(Cli::try_parse_from(["pmvid"]).is_err());
        assert!(Cli::try_parse_from(["pmvid", "-i", "in.mp3"]).is_err());
        assert!(Cli::try_parse_from(["pmvid", "-o", "out.mp4"]).is_err());
    }

    #[test]
    fn rejects_non_numeric_values() {
        assert!(Cli::try_parse_from([
            "pmvid", "-i", "in.mp3", "-o", "out.mp4", "--width", "wide"
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "pmvid", "-i", "in.mp3", "-o", "out.mp4", "--seed", "not-a-number"
        ])
        .is_err());
    }
}
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets cli
cargo test --all-targets
```

Manual check of help text:

```sh
cargo run --features projectm -- --help
```

This requires the native feature because the binary is gated. If projectM is
not built yet, `cargo test` is enough for this ticket.

## Acceptance criteria

- [ ] All PRD CLI flags are present with the documented short names.
- [ ] Defaults match PRD section 7.
- [ ] `-i` and `-o` are required.
- [ ] `Cli::command().debug_assert()` passes (no clap configuration errors).
- [ ] Non-numeric values are rejected by clap.

## Notes for the implementer

- Do not add validation here beyond types. `--width 0`, odd widths, `--crf 99`,
  and `--preset-duration 0` are handled in ticket `0014` so the error messages
  can be specific.
- Do not use `Option<PathBuf>` for input/output. Clap makes non-`Option` fields
  required automatically.
- Keep `--presets` default as `./presets` exactly; the PRD says so.
- If `-H` conflicts with clap's help flag on the version you use, check
  `Cli::command().debug_assert()` and report the conflict. Do not silently
  change the flag; the PRD specifies `-H`.
