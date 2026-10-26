# 0012 — ffmpeg encoder process

- **Milestone:** M1
- **Depends on:** 0011
- **Files:** `src/encoder.rs`, `tests/encoder_process.rs`
- **Feature:** default

## Goal

Spawn ffmpeg with the arguments from ticket `0011`, write raw RGBA frames to
its stdin, and finalize it cleanly. If ffmpeg exits early, return a useful error
instead of panicking with a broken pipe.

## Rules

- KISS: `std::process::Command`, no async runtime.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Add to `src/encoder.rs`:

```rust
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};

use anyhow::ensure;

/// A running ffmpeg process that accepts raw RGBA frames on stdin.
#[derive(Debug)]
pub struct Encoder {
    child: Child,
    stdin: Option<ChildStdin>,
    frame_bytes: usize,
    output: PathBuf,
}

impl Encoder {
    /// Spawn ffmpeg. The output file is written by ffmpeg, not by this process.
    pub fn spawn(config: &EncoderConfig) -> Result<Self> {
        let frame_bytes = config.frame_bytes()?;

        let mut child = Command::new(&config.ffmpeg)
            .args(config.args())
            .stdin(Stdio::piped())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn ffmpeg at '{}'; install ffmpeg or pass --ffmpeg <PATH>",
                    config.ffmpeg.display()
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .context("ffmpeg stdin was not piped")?;

        Ok(Self {
            child,
            stdin: Some(stdin),
            frame_bytes,
            output: config.output.clone(),
        })
    }

    /// Write exactly one RGBA frame to ffmpeg.
    pub fn write_frame(&mut self, frame: &[u8]) -> Result<()> {
        ensure!(
            frame.len() == self.frame_bytes,
            "frame has {} bytes, expected {}",
            frame.len(),
            self.frame_bytes
        );

        let stdin = self
            .stdin
            .as_mut()
            .context("ffmpeg stdin is already closed")?;
        stdin
            .write_all(frame)
            .context("writing frame to ffmpeg (did ffmpeg exit early?)")?;
        Ok(())
    }

    /// Close stdin and wait for ffmpeg to finish writing the output file.
    pub fn finish(mut self) -> Result<()> {
        drop(self.stdin.take());

        let status = self.child.wait().with_context(|| {
            format!("waiting for ffmpeg to finish '{}'", self.output.display())
        })?;

        ensure!(
            status.success(),
            "ffmpeg exited with {status}; output '{}' may be incomplete",
            self.output.display()
        );

        Ok(())
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // If a render error unwinds before finish(), do not leave ffmpeg running.
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
```

`finish` consumes `self`; `Drop` still runs after `finish` and calls `kill()` on
an already-exited child. That is harmless (the result is ignored).

## Tests

### `tests/encoder_process.rs`

These tests use a fake ffmpeg shell script, so they do not need a real ffmpeg
and they run quickly. They are Unix-only; Linux is the target platform.

```rust
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use pmvid::encoder::{Encoder, EncoderConfig};

fn fake_ffmpeg(dir: &Path, exit_code: i32) -> PathBuf {
    let path = dir.join("fake-ffmpeg.sh");
    let script = format!(
        r#"#!/bin/sh
out=""
for arg in "$@"; do out="$arg"; done
printf '%s\n' "$@" > "$out.args"
cat > "$out"
exit {exit_code}
"#
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn config(dir: &Path, ffmpeg: PathBuf, output: PathBuf) -> EncoderConfig {
    EncoderConfig {
        ffmpeg,
        input: dir.join("in.mp3"),
        output,
        width: 2,
        height: 2,
        fps: 1,
        crf: 20,
    }
}

#[test]
fn writes_frames_and_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(dir.path(), 0);
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), ffmpeg, output.clone());

    let mut encoder = Encoder::spawn(&cfg).unwrap();
    let frame = vec![0xAB_u8; cfg.frame_bytes().unwrap()];
    encoder.write_frame(&frame).unwrap();
    encoder.write_frame(&frame).unwrap();
    encoder.finish().unwrap();

    assert_eq!(
        fs::read(&output).unwrap().len(),
        2 * cfg.frame_bytes().unwrap()
    );

    let args = fs::read_to_string(output.with_extension("mp4.args")).unwrap();
    assert!(args.contains("-f\nrawvideo"));
    assert!(args.contains("-s\n2x2"));
    assert!(args.contains("-vf\nvflip"));
}

#[test]
fn rejects_a_frame_with_the_wrong_size() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(dir.path(), 0);
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), ffmpeg, output);
    let mut encoder = Encoder::spawn(&cfg).unwrap();

    let err = encoder.write_frame(&[0_u8; 3]).unwrap_err();
    assert!(format!("{err:#}").contains("expected 16"));
}

#[test]
fn reports_a_non_zero_ffmpeg_exit() {
    let dir = tempfile::tempdir().unwrap();
    let ffmpeg = fake_ffmpeg(dir.path(), 3);
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), ffmpeg, output);

    let mut encoder = Encoder::spawn(&cfg).unwrap();
    let frame = vec![0_u8; cfg.frame_bytes().unwrap()];
    encoder.write_frame(&frame).unwrap();

    let err = encoder.finish().unwrap_err();
    assert!(format!("{err:#}").contains("ffmpeg exited with"));
}

#[test]
fn reports_a_missing_ffmpeg_binary() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist-ffmpeg");
    let output = dir.path().join("out.mp4");
    let cfg = config(dir.path(), missing, output);

    let err = Encoder::spawn(&cfg).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("failed to spawn ffmpeg"));
    assert!(message.contains("install ffmpeg"));
}
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets encoder
cargo test --all-targets
```

## Acceptance criteria

- [ ] `Encoder::spawn` reports a clear error if ffmpeg cannot be started.
- [ ] `write_frame` rejects a buffer of the wrong size before writing.
- [ ] Frames written to the child arrive in order and with the correct length.
- [ ] `finish` closes stdin, waits, and reports a non-zero exit status.
- [ ] Dropping an encoder does not leave a child process running.
- [ ] Tests use a fake ffmpeg and do not require the real binary.

## Notes for the implementer

- Do not capture ffmpeg's stderr. It inherits the terminal, which is what users
  want when encoding fails. Capturing it would hide progress and warnings.
- `write_all` can fail with `BrokenPipe` if ffmpeg died. The context message
  tells the user to look at ffmpeg's output above.
- Do not add a thread for writing frames. ffmpeg's stdin buffer is enough for
  v1; if throughput becomes a problem, that is a later optimization.
- The fake script takes the **last** argument as the output path. That is why
  `EncoderConfig::args` must put the output last. If you reorder arguments,
  update the test helper.
