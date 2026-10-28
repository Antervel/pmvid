# 0022 — Preset switching and failed-preset handling

- **Milestone:** M2
- **Depends on:** 0021, 0010
- **Files:** `src/renderer.rs`, `tests/preset_failure.rs`
- **Feature:** `projectm`

## Goal

Make preset switching robust:

- The timed schedule from ticket `0010` already decides when to switch.
- projectM must not switch on its own (`set_preset_locked(true)` from `0019`).
- If a preset fails to compile, log a warning, keep rendering the previous
  preset, and continue. Do not abort the whole render.

## Rules

- KISS: one callback, one failure list, no retry logic.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below.

## Implementation

### 1. Add the failure callback to `src/renderer.rs`

Add imports:

```rust
use std::sync::{Arc, Mutex};
```

Add a field to `Renderer`:

```rust
pub struct Renderer {
    gl: GlContext,
    pm: ProjectM,
    fps: u32,
    failed_presets: Arc<Mutex<Vec<String>>>,
}
```

In `Renderer::new`, after `ProjectM::create()` and the NULL check, register the
callback **before** loading the first preset:

```rust
let failed_presets = Arc::new(Mutex::new(Vec::new()));
let failed_for_callback = Arc::clone(&failed_presets);

pm.set_preset_switch_failed_event_callback(move |filename, message| {
    eprintln!("warning: preset '{filename}' failed to load: {message}");
    if let Ok(mut failures) = failed_for_callback.lock() {
        failures.push(filename);
    }
});
```

Store `failed_presets` in the `Self` value. The construction at the end of
`Renderer::new` becomes:

```rust
let renderer = Self {
    gl,
    pm,
    fps: config.fps,
    failed_presets,
};
renderer.load_preset(preset, false)?;
Ok(renderer)
```

### 2. Make `load_preset` check the path

```rust
pub fn load_preset(&self, preset: &Path, smooth: bool) -> Result<()> {
    ensure!(
        preset.is_file(),
        "preset file '{}' does not exist",
        preset.display()
    );
    let preset = preset.to_str().with_context(|| {
        format!("preset path is not valid UTF-8: {}", preset.display())
    })?;
    self.pm.load_preset_file(preset, smooth);
    Ok(())
}
```

### 3. Add a read-only failure list

```rust
/// Presets that projectM reported as failed since startup.
pub fn failed_presets(&self) -> Vec<String> {
    self.failed_presets
        .lock()
        .map(|failures| failures.clone())
        .unwrap_or_default()
}
```

Do not remove entries. A failed preset may be retried at the next cycle; seeing
it again is useful.

## Tests

### `tests/preset_failure.rs`

```rust
#![cfg(feature = "projectm")]

use std::path::{Path, PathBuf};

use pmvid::config::RenderConfig;
use pmvid::renderer::Renderer;

fn config() -> RenderConfig {
    RenderConfig {
        input: PathBuf::from("unused.mp3"),
        output: PathBuf::from("unused.mp4"),
        width: 64,
        height: 64,
        fps: 30,
        presets: PathBuf::from("tests/fixtures"),
        preset_duration: 20.0,
        beat_sensitivity: 1.0,
        seed: 0,
        crf: 20,
        ffmpeg: PathBuf::from("ffmpeg"),
    }
}

#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn records_a_broken_preset_and_keeps_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.milk");
    std::fs::write(&broken, b"this is not a milkdrop preset").unwrap();

    let mut renderer = Renderer::new(&config(), Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");

    renderer
        .load_preset(&broken, false)
        .expect("load_preset should not fail for a compile error");

    // Render a frame so any deferred failure is processed.
    let mut frame = vec![0_u8; renderer.frame_bytes()];
    renderer.render_frame(0.0, &mut frame).unwrap();

    let failures = renderer.failed_presets();
    assert!(
        failures.iter().any(|name| name.contains("broken.milk")),
        "expected broken.milk in failures, got {failures:?}"
    );
}

#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn missing_preset_file_is_an_error() {
    let renderer = Renderer::new(&config(), Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");

    let err = renderer
        .load_preset(Path::new("tests/fixtures/does-not-exist.milk"), false)
        .unwrap_err();
    assert!(format!("{err:#}").contains("does not exist"));
}
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture
```

Manual test:

```sh
mkdir -p /tmp/bad-presets
cp tests/fixtures/simple.milk /tmp/bad-presets/good.milk
printf 'not a preset\n' > /tmp/bad-presets/broken.milk
cargo run --features projectm -- \
  -i /path/to/short.mp3 -o /tmp/bad.mp4 --presets /tmp/bad-presets --preset-duration 1
```

Expected: a warning about `broken.milk`, the render finishes, and the MP4 plays.

## Acceptance criteria

- [x] projectM's own preset timer cannot switch presets.
- [x] A failed preset logs a warning and does not abort the render.
- [x] Failed preset names are recorded in `failed_presets()`.
- [x] A missing preset file returns an error from `load_preset`.
- [ ] The ignored native test passes manually.

## Notes for the implementer

- The projectM callback is `FnMut(String, String) + 'static`. The wrapper leaks
  the boxed closure, which is fine for one long-lived renderer.
- `projectm_load_preset_file` returns `void`; a compile failure is reported via
  the callback. There is no `Result` to check.
- Do not try to "repair" a failed preset. Skip it and continue. The schedule
  will move to the next preset at the next timed switch.
- If the callback is not called before the first `render_frame`, the ignored
  test may need one extra render call. Add it if necessary, but do not add
  sleeps.
- Keep the warning on stderr, not stdout. stdout is reserved for normal output.

## Deviation from the plan

- `Renderer::load_preset` now hands projectM a NUL-terminated C string
  (`std::ffi::CString`) instead of a raw `&str`, because projectM's FFI takes a
  C string and a `&str` borrowed from `Path::to_str()` is not guaranteed to be
  NUL-terminated.
- The vendored projectM callback trampolines no longer `unwrap()` the callback
  strings; they fall back to an empty string. The trampolines are `extern "C"`
  and cannot unwind, so a bad string would abort the whole render.
