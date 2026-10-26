# 0015 — Progress reporting

- **Milestone:** M2
- **Depends on:** 0001
- **Files:** `src/progress.rs`, `src/lib.rs`
- **Feature:** default

## Goal

Show a progress bar with elapsed time and ETA during rendering, and stay silent
when stderr is not a terminal (CI, pipes, tests).

## Rules

- KISS: wrap `indicatif::ProgressBar`; no custom drawing.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Create `src/progress.rs`:

```rust
//! Optional progress reporting.

use indicatif::{ProgressBar, ProgressStyle};

/// A progress bar over video frames.
pub struct Progress {
    bar: ProgressBar,
}

impl Progress {
    /// Create a visible progress bar on stderr.
    pub fn new(total_frames: u64, fps: u32) -> Self {
        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        )
        .expect("static progress template is valid")
        .progress_chars("#>-");

        let bar = ProgressBar::new(total_frames);
        bar.set_style(style);
        bar.set_message(format!("{fps} fps"));
        Self { bar }
    }

    /// Create a no-op progress bar for non-interactive output.
    pub fn hidden() -> Self {
        Self {
            bar: ProgressBar::hidden(),
        }
    }

    /// Update the current frame. Safe to call with values above the total.
    pub fn set_frame(&self, frame: u64) {
        self.bar.set_position(frame);
    }

    /// Clear the bar when rendering finishes.
    pub fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_progress_does_not_panic() {
        let progress = Progress::hidden();
        progress.set_frame(0);
        progress.set_frame(100);
        progress.finish();
    }

    #[test]
    fn setting_a_frame_past_the_total_does_not_panic() {
        let progress = Progress::hidden();
        progress.set_frame(u64::MAX);
        progress.finish();
    }
}
```

Register the module:

```rust
pub mod progress;
```

In the render loop (ticket `0021`), choose visible or hidden like this:

```rust
use std::io::IsTerminal;

let progress = if std::io::stderr().is_terminal() {
    Progress::new(total_frames, config.fps)
} else {
    Progress::hidden()
};
```

## Tests

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets progress
cargo test --all-targets
```

Manual visual check (after ticket `0021`):

```sh
cargo run --features projectm -- -i song.mp3 -o song.mp4
```

## Acceptance criteria

- [ ] `Progress::hidden()` produces no output and never panics.
- [ ] `Progress::new` uses a static template and does not allocate per frame.
- [ ] `set_frame` and `finish` are safe to call in any order.
- [ ] The app can choose hidden mode based on `stderr().is_terminal()`.

## Notes for the implementer

- Do not add a `--quiet` flag. Terminal detection covers CI and pipes, which is
  the actual need. If a user asks for quiet mode later, add it then.
- Do not call `bar.inc()` once per frame from multiple threads. The render loop
  is single-threaded.
- `expect` on the static template is acceptable: the string is a compile-time
  constant and a failure means the code is broken, not user input.
- The ETA is based on frame count and elapsed wall time. It is accurate enough
  for a progress bar.
