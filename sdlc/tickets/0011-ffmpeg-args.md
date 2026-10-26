# 0011 — ffmpeg argument builder

- **Milestone:** M1
- **Depends on:** 0001
- **Files:** `src/encoder.rs`, `src/lib.rs`
- **Feature:** default

## Goal

Build the ffmpeg command line for raw RGBA frames on stdin plus the original
audio file on disk. Keep argument construction pure and unit-testable; ticket
`0012` actually spawns the process.

The filter `vflip` is done by ffmpeg because OpenGL's origin is bottom-left.
Doing it here avoids a CPU row copy in the render loop.

## Rules

- KISS: one config struct, one `args()` method.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Create `src/encoder.rs`:

```rust
//! ffmpeg process management.

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Everything needed to build the ffmpeg command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderConfig {
    pub ffmpeg: PathBuf,
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub crf: u8,
}

impl EncoderConfig {
    /// Number of bytes in one RGBA frame.
    pub fn frame_bytes(&self) -> Result<usize> {
        let pixels = (self.width as usize)
            .checked_mul(self.height as usize)
            .context("video resolution is too large")?;
        pixels
            .checked_mul(4)
            .context("video resolution is too large")
    }

    /// Arguments passed to ffmpeg. Does not include the program path.
    pub fn args(&self) -> Vec<OsString> {
        let mut args = Vec::new();

        args.push("-y".into());
        args.push("-f".into());
        args.push("rawvideo".into());
        args.push("-pix_fmt".into());
        args.push("rgba".into());
        args.push("-s".into());
        args.push(format!("{}x{}", self.width, self.height).into());
        args.push("-r".into());
        args.push(self.fps.to_string().into());
        args.push("-i".into());
        args.push("-".into());

        args.push("-i".into());
        args.push(self.input.as_os_str().to_owned());

        args.push("-map".into());
        args.push("0:v:0".into());
        args.push("-map".into());
        args.push("1:a:0".into());

        args.push("-c:v".into());
        args.push("libx264".into());
        args.push("-pix_fmt".into());
        args.push("yuv420p".into());
        args.push("-crf".into());
        args.push(self.crf.to_string().into());

        // OpenGL reads pixels bottom-up. Let ffmpeg flip them.
        args.push("-vf".into());
        args.push("vflip".into());

        args.push("-c:a".into());
        args.push("aac".into());
        args.push("-b:a".into());
        args.push("192k".into());
        args.push("-shortest".into());

        args.push(self.output.as_os_str().to_owned());

        args
    }
}
```

Register the module:

```rust
pub mod encoder;
```

## Tests

Add to `src/encoder.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> EncoderConfig {
        EncoderConfig {
            ffmpeg: "ffmpeg".into(),
            input: "song.mp3".into(),
            output: "song.mp4".into(),
            width: 1280,
            height: 720,
            fps: 30,
            crf: 20,
        }
    }

    #[test]
    fn builds_the_expected_argument_list() {
        let args: Vec<String> = config()
            .args()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        let expected = vec![
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            "1280x720",
            "-r",
            "30",
            "-i",
            "-",
            "-i",
            "song.mp3",
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "20",
            "-vf",
            "vflip",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-shortest",
            "song.mp4",
        ];

        assert_eq!(args, expected);
    }

    #[test]
    fn frame_size_is_width_times_height_times_four() {
        assert_eq!(config().frame_bytes().unwrap(), 1280 * 720 * 4);
    }

    #[test]
    fn frame_size_uses_the_actual_resolution() {
        let cfg = EncoderConfig {
            width: 854,
            height: 480,
            ..config()
        };
        assert_eq!(cfg.frame_bytes().unwrap(), 854 * 480 * 4);
    }

    #[test]
    fn crf_and_fps_are_stringified() {
        let cfg = EncoderConfig {
            fps: 24,
            crf: 18,
            ..config()
        };
        let args: Vec<String> = cfg
            .args()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|pair| pair == ["-r", "24"]));
        assert!(args.windows(2).any(|pair| pair == ["-crf", "18"]));
    }
}
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets encoder
cargo test --all-targets
```

Manual sanity check with a real ffmpeg (optional):

```sh
ffmpeg -hide_banner -f rawvideo -pix_fmt rgba -s 2x2 -r 1 -i /dev/zero \
  -i /tmp/silence.wav -map 0:v:0 -map 1:a:0 -c:v libx264 -pix_fmt yuv420p \
  -crf 20 -vf vflip -c:a aac -b:a 192k -shortest -y /tmp/test.mp4
```

## Acceptance criteria

- [ ] The argument list matches the expected vector exactly.
- [ ] The command maps video from stdin (`0:v:0`) and audio from the input file
      (`1:a:0`).
- [ ] The output is H.264 (`libx264`, `yuv420p`) with AAC audio.
- [ ] `-vf vflip` is present.
- [ ] `frame_bytes()` returns `width * height * 4` and reports overflow.

## Notes for the implementer

- Keep paths as `PathBuf`/`OsString`; do not convert them to `String`. Linux
  paths do not have to be UTF-8.
- Do not add `-preset`, `-movflags`, or hardware encoders here. Those are
  later/optional.
- `-shortest` makes ffmpeg stop at the shorter stream. Since the frame count is
  derived from the audio length, this is a safety net.
- `-vf vflip` means the renderer must **not** flip rows in Rust. Ticket `0021`
  relies on this.
