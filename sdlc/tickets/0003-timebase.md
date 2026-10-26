# 0003 — Frame/sample timebase math

- **Milestone:** M1
- **Depends on:** 0001
- **Files:** `src/timebase.rs`, `src/lib.rs`
- **Feature:** default

## Goal

Implement the integer math that maps a video frame number to a range of audio
samples. This is the foundation of A/V sync and must be exactly reproducible.

At 44,100 Hz and 30 fps there are exactly 1,470 samples per frame. At 24 fps
there are 1,837.5, so a rounded `samples_per_frame` constant would drift. Use
cumulative positions instead:

```text
start = frame * SAMPLE_RATE / fps
end   = (frame + 1) * SAMPLE_RATE / fps
```

The division is integer division, and the multiplication happens first.

## Rules

- KISS: no floating point in the sample-range calculation.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

### 1. Create `src/timebase.rs`

```rust
//! Integer timebase math shared by the audio and render loops.

use std::ops::Range;

use crate::SAMPLE_RATE;

/// Number of video frames needed to cover `total_samples_per_channel` audio.
///
/// The result is rounded down, so the last frame never reads past the end of
/// the decoded PCM buffer.
pub fn total_frames(total_samples_per_channel: u64, fps: u32) -> u64 {
    debug_assert!(fps > 0, "fps must be positive");
    total_samples_per_channel * u64::from(fps) / u64::from(SAMPLE_RATE)
}

/// Half-open range of audio samples (per channel) for a video frame.
///
/// The ranges for consecutive frames are contiguous:
/// `frame_sample_range(n).end == frame_sample_range(n + 1).start`.
pub fn frame_sample_range(frame: u64, fps: u32) -> Range<u64> {
    debug_assert!(fps > 0, "fps must be positive");
    let rate = u64::from(SAMPLE_RATE);
    let fps = u64::from(fps);
    let start = frame * rate / fps;
    let end = (frame + 1) * rate / fps;
    start..end
}

/// Presentation time of a frame, in seconds, for projectM's fixed frame time.
pub fn frame_time_seconds(frame: u64, fps: u32) -> f64 {
    debug_assert!(fps > 0, "fps must be positive");
    frame as f64 / f64::from(fps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_fps_has_1470_samples_per_frame() {
        assert_eq!(frame_sample_range(0, 30), 0..1_470);
        assert_eq!(frame_sample_range(1, 30), 1_470..2_940);
        assert_eq!(frame_sample_range(29, 30), 42_630..44_100);
    }

    #[test]
    fn twenty_four_fps_ranges_are_contiguous() {
        let mut previous_end = 0;
        for frame in 0..100 {
            let range = frame_sample_range(frame, 24);
            assert_eq!(range.start, previous_end, "gap before frame {frame}");
            assert!(range.end > range.start, "empty range at frame {frame}");
            previous_end = range.end;
        }
    }

    #[test]
    fn ten_minutes_at_thirty_fps_has_no_drift() {
        // 10 minutes * 60 s * 44,100 Hz = 26,460,000 samples.
        let total_samples = 26_460_000;
        let frames = total_frames(total_samples, 30);
        assert_eq!(frames, 18_000);
        assert_eq!(frame_sample_range(frames - 1, 30).end, total_samples);
    }

    #[test]
    fn ten_minutes_at_twenty_four_fps_has_no_drift() {
        let total_samples = 26_460_000;
        let frames = total_frames(total_samples, 24);
        assert_eq!(frames, 14_400);
        assert_eq!(frame_sample_range(frames - 1, 24).end, total_samples);
    }

    #[test]
    fn total_frames_rounds_down() {
        // 1.5 frames at 30 fps is one complete frame.
        assert_eq!(total_frames(2_205, 30), 1);
        // Less than one frame of audio is zero frames.
        assert_eq!(total_frames(100, 30), 0);
        assert_eq!(total_frames(0, 30), 0);
    }

    #[test]
    fn frame_time_is_linear() {
        assert_eq!(frame_time_seconds(0, 30), 0.0);
        assert!((frame_time_seconds(30, 30) - 1.0).abs() < 1e-12);
        assert!((frame_time_seconds(15, 30) - 0.5).abs() < 1e-12);
    }
}
```

### 2. Register the module in `src/lib.rs`

```rust
pub mod timebase;
```

Place it next to the existing constants. Do not re-export the functions yet;
callers can use `pmvid::timebase::...`.

## Tests

The unit tests above are the deliverable.

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets timebase
cargo test --all-targets
```

Expected: all pass.

## Acceptance criteria

- [x] `frame_sample_range` uses only integer arithmetic.
- [x] Consecutive frame ranges are contiguous at 24, 30, and 60 fps.
- [x] A 10-minute track has zero cumulative drift at 24 and 30 fps.
- [x] `total_frames` never produces a last frame that reads past the PCM end.
- [x] Tests cover 30 fps, 24 fps, a partial frame, and empty input.

## Notes for the implementer

- Do **not** add a `samples_per_frame` field. It is tempting and it is exactly
  the bug this ticket prevents.
- `fps` is a `u32` from the CLI. The PRD mentions 29.97 fps, but the CLI accepts
  an integer; do not add fractional-fps support in this ticket.
- If you later need nanoseconds or rational time, add a separate function. Do
  not change these signatures without updating every caller and test.
