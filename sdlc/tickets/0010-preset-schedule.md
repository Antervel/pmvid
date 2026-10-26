# 0010 — Timed preset schedule

- **Milestone:** M1
- **Depends on:** 0008, 0009
- **Files:** `src/presets.rs`
- **Feature:** default

## Goal

Track which preset is active and when to switch to the next one. Switching is
time-based: every `preset_duration` seconds, advance to the next preset in the
shuffled list, wrapping around.

This ticket is pure logic and must be testable without projectM.

## Rules

- KISS: one struct, one `advance` method.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Add to `src/presets.rs`:

Extend the existing `anyhow` import from ticket `0008` so it reads:

```rust
use anyhow::{bail, ensure, Context, Result};
```

Do not add a second `use anyhow::...;` line; that would be a duplicate
definition error for `Result`.

Then add the struct:

```rust
/// Time-based preset switching state.
#[derive(Debug)]
pub struct PresetSchedule {
    presets: Vec<PathBuf>,
    frames_per_preset: u64,
    index: usize,
    next_switch_frame: u64,
}

impl PresetSchedule {
    /// Create a schedule that switches every `preset_duration_s` seconds.
    pub fn new(presets: Vec<PathBuf>, preset_duration_s: f64, fps: u32) -> Result<Self> {
        ensure!(!presets.is_empty(), "cannot schedule an empty preset list");
        ensure!(fps > 0, "fps must be greater than zero");
        ensure!(
            preset_duration_s.is_finite() && preset_duration_s > 0.0,
            "preset duration must be a positive number of seconds"
        );

        let frames_per_preset = (preset_duration_s * f64::from(fps)).round() as u64;
        ensure!(
            frames_per_preset > 0,
            "preset duration is shorter than one frame at {fps} fps"
        );

        Ok(Self {
            presets,
            frames_per_preset,
            index: 0,
            next_switch_frame: frames_per_preset,
        })
    }

    /// The preset that should currently be loaded.
    pub fn current(&self) -> &Path {
        &self.presets[self.index]
    }

    /// Number of frames each preset is shown for.
    pub fn frames_per_preset(&self) -> u64 {
        self.frames_per_preset
    }

    /// Advance if `frame` has reached the next switch point.
    ///
    /// Returns the new preset path when a switch happened, or `None` otherwise.
    /// Call this once per frame, before rendering.
    pub fn advance(&mut self, frame: u64) -> Option<&Path> {
        if frame < self.next_switch_frame {
            return None;
        }

        self.index = (self.index + 1) % self.presets.len();
        self.next_switch_frame += self.frames_per_preset;
        Some(self.current())
    }
}
```

The first preset is loaded before the loop starts. The first switch happens at
frame `frames_per_preset`, not at frame 0.

## Tests

Add to the `tests` module in `src/presets.rs`:

```rust
fn schedule(names: &[&str], seconds: f64, fps: u32) -> PresetSchedule {
    PresetSchedule::new(paths(names), seconds, fps).unwrap()
}

#[test]
fn starts_on_the_first_preset() {
    let schedule = schedule(&["a.milk", "b.milk"], 20.0, 30);
    assert_eq!(schedule.current(), Path::new("a.milk"));
    assert_eq!(schedule.frames_per_preset(), 600);
}

#[test]
fn switches_at_the_expected_frame() {
    let mut schedule = schedule(&["a.milk", "b.milk"], 20.0, 30);

    assert!(schedule.advance(0).is_none());
    assert!(schedule.advance(599).is_none());
    assert_eq!(schedule.advance(600), Some(Path::new("b.milk")));
    assert_eq!(schedule.current(), Path::new("b.milk"));

    assert!(schedule.advance(1_199).is_none());
    assert_eq!(schedule.advance(1_200), Some(Path::new("a.milk")));
}

#[test]
fn wraps_around() {
    let mut schedule = schedule(&["a.milk", "b.milk", "c.milk"], 1.0, 30);
    assert_eq!(schedule.advance(30), Some(Path::new("b.milk")));
    assert_eq!(schedule.advance(60), Some(Path::new("c.milk")));
    assert_eq!(schedule.advance(90), Some(Path::new("a.milk")));
}

#[test]
fn single_preset_repeats_itself() {
    let mut schedule = schedule(&["only.milk"], 1.0, 30);
    assert_eq!(schedule.advance(30), Some(Path::new("only.milk")));
    assert_eq!(schedule.advance(60), Some(Path::new("only.milk")));
}

#[test]
fn duration_shorter_than_one_frame_is_rejected() {
    let err = PresetSchedule::new(paths(&["a.milk"]), 0.001, 30).unwrap_err();
    assert!(format!("{err:#}").contains("shorter than one frame"));
}

#[test]
fn invalid_inputs_are_rejected() {
    assert!(PresetSchedule::new(Vec::new(), 20.0, 30).is_err());
    assert!(PresetSchedule::new(paths(&["a.milk"]), 0.0, 30).is_err());
    assert!(PresetSchedule::new(paths(&["a.milk"]), f64::NAN, 30).is_err());
    assert!(PresetSchedule::new(paths(&["a.milk"]), 20.0, 0).is_err());
}

#[test]
fn non_integer_frames_round_to_nearest() {
    // 2.5 s at 24 fps = 60 frames exactly.
    assert_eq!(schedule(&["a.milk"], 2.5, 24).frames_per_preset(), 60);
    // 2.55 s at 30 fps = 76.5 -> 77 frames.
    assert_eq!(schedule(&["a.milk"], 2.55, 30).frames_per_preset(), 77);
}
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets presets
cargo test --all-targets
```

## Acceptance criteria

- [ ] The first preset is active at frame 0.
- [ ] The first switch happens exactly at `frames_per_preset`.
- [ ] The schedule wraps to the first preset after the last one.
- [ ] A single preset is re-selected without panicking.
- [ ] Invalid durations and fps values are rejected.

## Notes for the implementer

- `advance` only advances one step even if the caller skips many frames. The
  render loop calls it every frame, so this is fine. If you later add seeking,
  change it to a loop and add a test.
- Do not use wall-clock time here. Frames are the source of truth.
- Do not call `projectm_set_preset_duration` and rely on projectM to switch.
  We control switching so that it is deterministic and testable.
