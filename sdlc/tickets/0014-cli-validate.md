# 0014 — CLI validation

- **Milestone:** M1
- **Depends on:** 0013
- **Files:** `src/cli.rs`
- **Feature:** default

## Goal

Reject values that would make ffmpeg or the renderer fail later, with a clear
message. Keep validation separate from parsing so it can be unit-tested.

## Rules

- KISS: a single `validate()` method with `ensure!` checks.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Add to `src/cli.rs`:

```rust
use anyhow::{ensure, Result};

impl Cli {
    /// Validate values that clap cannot express cleanly.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.width > 0 && self.height > 0,
            "width and height must be greater than zero"
        );
        ensure!(
            self.width % 2 == 0 && self.height % 2 == 0,
            "width and height must be even for H.264 yuv420p (got {}x{})",
            self.width,
            self.height
        );
        ensure!(self.fps > 0, "fps must be greater than zero");
        ensure!(
            self.preset_duration.is_finite() && self.preset_duration > 0.0,
            "preset duration must be a positive number of seconds"
        );
        ensure!(
            self.beat_sensitivity.is_finite() && self.beat_sensitivity > 0.0,
            "beat sensitivity must be a positive number"
        );
        ensure!(self.crf <= 51, "crf must be between 0 and 51");
        Ok(())
    }
}
```

Do not add a check for an empty `--ffmpeg` value: clap's `PathBuf` value parser
already rejects `--ffmpeg ""` during parsing, so a `validate` check for it would
be unreachable and untestable.

Do not check that the input file exists here. Ticket `0016` does file-system
checks in `RenderConfig::from_cli`, where the error can include the resolved
path.

## Tests

Add to the `tests` module in `src/cli.rs`:

```rust
fn valid_args() -> Vec<&'static str> {
    vec!["pmvid", "-i", "song.mp3", "-o", "song.mp4"]
}

#[test]
fn defaults_are_valid() {
    let cli = parse(&valid_args());
    cli.validate().unwrap();
}

#[test]
fn accepts_a_non_16_by_9_even_resolution() {
    let cli = parse(&[
        "pmvid", "-i", "in.mp3", "-o", "out.mp4", "-W", "854", "-H", "480",
    ]);
    cli.validate().unwrap();
}

#[test]
fn rejects_zero_width_or_height() {
    let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-W", "0"]);
    assert!(cli.validate().is_err());

    let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-H", "0"]);
    assert!(cli.validate().is_err());
}

#[test]
fn rejects_odd_dimensions() {
    let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-W", "1279"]);
    let err = cli.validate().unwrap_err();
    assert!(format!("{err:#}").contains("must be even"));

    let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-H", "721"]);
    assert!(cli.validate().is_err());
}

#[test]
fn rejects_zero_fps() {
    let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-f", "0"]);
    let err = cli.validate().unwrap_err();
    assert!(format!("{err:#}").contains("fps must be greater than zero"));
}

#[test]
fn rejects_bad_preset_duration() {
    // Use the `--flag=value` form: clap treats a separate `-1` token as an
    // unknown flag and would reject it during parsing, before `validate`.
    for value in ["0", "-1", "NaN", "inf"] {
        let arg = format!("--preset-duration={value}");
        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", &arg]);
        assert!(cli.validate().is_err(), "accepted duration {value}");
    }
}

#[test]
fn rejects_bad_beat_sensitivity() {
    for value in ["0", "-0.5", "NaN", "inf"] {
        let arg = format!("--beat-sensitivity={value}");
        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", &arg]);
        assert!(cli.validate().is_err(), "accepted sensitivity {value}");
    }
}

#[test]
fn rejects_crf_above_51() {
    let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "--crf", "52"]);
    let err = cli.validate().unwrap_err();
    assert!(format!("{err:#}").contains("crf must be between 0 and 51"));
}

#[test]
fn rejects_empty_ffmpeg_path() {
    // clap's `PathBuf` value parser already rejects an empty value, so this
    // fails during parsing rather than in `validate`.
    assert!(Cli::try_parse_from([
        "pmvid", "-i", "in.mp3", "-o", "out.mp4", "--ffmpeg", ""
    ])
    .is_err());
}
```

Note that `parse(&[..., &arg])` works because `&String` coerces to `&str` in
the array literal.

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets cli
cargo test --all-targets
```

## Acceptance criteria

- [ ] Defaults pass validation.
- [ ] Zero or odd width/height are rejected with a message mentioning evenness.
- [ ] Zero fps is rejected.
- [ ] `NaN`, infinite, zero, and negative preset durations are rejected.
- [ ] `NaN`, infinite, zero, and negative beat sensitivities are rejected.
- [ ] CRF above 51 is rejected.
- [ ] An empty ffmpeg path is rejected by clap.

## Notes for the implementer

- H.264 with `yuv420p` requires even dimensions. This is why odd sizes are
  rejected even though ffmpeg might sometimes accept them.
- Do not clamp invalid values silently. The user should know their input was
  wrong.
- Keep `validate` free of filesystem access so it stays a pure unit test.
- In tests, pass negative numbers as `--preset-duration=-1`, not as a separate
  `-1` token. Without `allow_negative_numbers`, clap treats a leading `-` token
  as an unknown flag and fails during parsing.
