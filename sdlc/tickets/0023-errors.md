# 0023 — Error handling and exit codes

- **Milestone:** M3
- **Depends on:** 0021
- **Files:** `src/app.rs`, `src/main.rs`, `tests/failure_modes.rs`,
  `tests/cli_binary_errors.rs`
- **Feature:** `projectm` for the binary tests; default for library tests

## Goal

Make every failure path produce a clear message and a non-zero exit code:

- missing input file
- corrupt/unreadable audio
- empty or missing preset directory
- missing ffmpeg
- no EGL/GL context
- ffmpeg exits non-zero
- bad CLI values

Most contexts already exist from earlier tickets. This ticket audits them,
adds binary-level tests, and makes sure `main` prints the full `anyhow` chain.

## Rules

- KISS: use `anyhow::Context` and `ensure!`; no custom error enum.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below.

## Implementation

### 1. Audit `src/main.rs`

It should look exactly like this:

```rust
fn main() {
    if let Err(err) = pmvid::app::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
```

`{err:#}` prints the whole context chain on one line:
`error: starting ffmpeg: failed to spawn ffmpeg at 'ffmpeg': ...`

### 2. Audit `src/app.rs` contexts

Each fallible step must have a context that names the stage:

```rust
let audio = audio::prepare(&config.input).context("preparing audio")?;
let preset_files = presets::scan(&config.presets).context("loading presets")?;
let mut renderer = Renderer::new(&config, schedule.current())
    .context("initializing the renderer")?;
let mut encoder = Encoder::spawn(&config.encoder_config())
    .context("starting ffmpeg")?;
```

Do not replace the existing specific messages inside those functions. The
context adds the stage; the inner message says what to fix.

### 3. Add library-level failure tests

`tests/failure_modes.rs`:

```rust
use std::fs;
use std::path::PathBuf;

use clap::Parser;
use pmvid::audio::prepare;
use pmvid::cli::Cli;
use pmvid::config::RenderConfig;
use pmvid::encoder::{Encoder, EncoderConfig};
use pmvid::presets::scan;

#[test]
fn missing_input_file_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "pmvid",
        "-i",
        dir.path().join("missing.mp3").to_str().unwrap(),
        "-o",
        dir.path().join("out.mp4").to_str().unwrap(),
    ])
    .unwrap();

    let err = RenderConfig::from_cli(cli).unwrap_err();
    assert!(format!("{err:#}").contains("input file"));
}

#[test]
fn corrupt_audio_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.mp3");
    fs::write(&path, b"not audio").unwrap();

    let err = prepare(&path).unwrap_err();
    assert!(format!("{err:#}").contains("unsupported or corrupt audio file"));
}

#[test]
fn empty_preset_directory_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();

    let err = scan(dir.path()).unwrap_err();
    assert!(format!("{err:#}").contains("no .milk presets found"));
}

#[test]
fn missing_ffmpeg_has_a_clear_error() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = EncoderConfig {
        ffmpeg: dir.path().join("no-such-ffmpeg"),
        input: PathBuf::from("in.mp3"),
        output: dir.path().join("out.mp4"),
        width: 64,
        height: 64,
        fps: 30,
        crf: 20,
    };

    let err = Encoder::spawn(&cfg).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("failed to spawn ffmpeg"));
    assert!(message.contains("install ffmpeg"));
}
```

### 4. Add binary-level tests

`tests/cli_binary_errors.rs`:

```rust
#![cfg(feature = "projectm")]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn no_arguments_fails_with_usage() {
    Command::cargo_bin("pmvid")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn missing_input_file_fails() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.mp4");

    Command::cargo_bin("pmvid")
        .unwrap()
        .args([
            "-i",
            dir.path().join("missing.mp3").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("input file"));
}

#[test]
fn corrupt_audio_fails_before_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("broken.mp3");
    fs::write(&input, b"not audio").unwrap();
    let output = dir.path().join("out.mp4");

    Command::cargo_bin("pmvid")
        .unwrap()
        .args([
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--presets",
            "tests/fixtures",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported or corrupt audio file"));
}
```

These tests do not reach the GL renderer, but they do link the binary, so they
require `--features projectm`. They are not ignored because they should work on
any machine that can build the feature.

### 5. Manual failure checks

Add these to the README/troubleshooting in ticket `0025`:

```sh
# no EGL/GL
env -u DISPLAY -u WAYLAND_DISPLAY pmvid -i song.mp3 -o out.mp4 --presets ./presets
# expected: "initializing the renderer: ... EGL ..." and exit 1

# ffmpeg missing
pmvid -i song.mp3 -o out.mp4 --presets ./presets --ffmpeg /no/such/ffmpeg
# expected: "starting ffmpeg: failed to spawn ffmpeg ... install ffmpeg"

# empty presets
mkdir -p /tmp/empty-presets
pmvid -i song.mp3 -o out.mp4 --presets /tmp/empty-presets
# expected: "loading presets: no .milk presets found ..."
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --all-targets failure_modes

cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
```

## Acceptance criteria

- [ ] `main` prints `error: <full context chain>` and exits 1.
- [ ] Missing input, corrupt audio, empty presets, and missing ffmpeg have
      tests asserting on the message.
- [ ] Binary-level tests assert a non-zero exit code.
- [ ] No failure path panics with `unwrap()` on user input.
- [ ] The manual no-EGL and missing-ffmpeg checks are documented.

## Notes for the implementer

- Do not add a custom `Error` enum. `anyhow` is enough for a CLI.
- Do not catch errors in `main` and print `{err}` (without `#`); that hides the
  context chain.
- `assert_cmd` tests that call `Command::cargo_bin("pmvid")` need the binary to
  be built. With `required-features = ["projectm"]`, that means running the
  tests with `--features projectm`.
- If `Command::cargo_bin` is deprecated in the resolved `assert_cmd` version,
  use the recommended replacement from its docs, but keep the same assertions.
- Do not test GL failures in unit tests; they depend on the machine. Use the
  manual command above.
