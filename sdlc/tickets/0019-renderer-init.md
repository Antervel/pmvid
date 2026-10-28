# 0019 — Renderer initialization and one frame

- **Milestone:** M1
- **Depends on:** 0017, 0018, 0008, 0010
- **Files:** `src/renderer.rs`, `src/lib.rs`, `tests/renderer_smoke.rs`
- **Feature:** `projectm`

## Goal

Wrap the GL context and projectM handle in a small `Renderer` that can:

1. initialize projectM after the GL context is current,
2. load a preset,
3. render one frame into the offscreen FBO at a fixed frame time,
4. read the frame back as RGBA.

No audio loop yet. This ticket proves the render path.

## Rules

- KISS: one `Renderer` struct, no traits, no backend abstraction.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below.

## Implementation

### 1. Create `src/renderer.rs`

```rust
//! projectM renderer: GL context + projectM instance + FBO readback.

use std::path::Path;

use anyhow::{ensure, Context, Result};
use projectm::core::ProjectM;

use crate::config::RenderConfig;
use crate::gl_ctx::GlContext;

/// Owns the GL context and projectM instance.
///
/// The GL context is created first and stays current for the lifetime of this
/// struct. Keep all rendering on the thread that constructed it.
pub struct Renderer {
    gl: GlContext,
    pm: ProjectM,
    fps: u32,
}

impl Renderer {
    /// Create the GL context, then projectM, then load the first preset.
    pub fn new(config: &RenderConfig, preset: &Path) -> Result<Self> {
        let gl = GlContext::new(config.width, config.height)
            .context("creating the headless OpenGL context")?;

        let pm = ProjectM::create();
        let handle = *pm.get_instance().borrow();
        ensure!(
            !handle.is_null(),
            "projectM failed to create an instance; the GL context may not be current"
        );

        pm.set_window_size(config.width as usize, config.height as usize);
        pm.set_fps(config.fps);
        pm.set_beat_sensitivity(config.beat_sensitivity);

        // We drive preset switching from the frame schedule. Stop projectM's
        // own timer from requesting switches.
        pm.set_preset_locked(true);

        let renderer = Self {
            gl,
            pm,
            fps: config.fps,
        };
        renderer.load_preset(preset, false)?;
        Ok(renderer)
    }

    /// Load a preset. `smooth` requests projectM's soft cut.
    pub fn load_preset(&self, preset: &Path, smooth: bool) -> Result<()> {
        let preset = preset.to_str().with_context(|| {
            format!("preset path is not valid UTF-8: {}", preset.display())
        })?;
        self.pm.load_preset_file(preset, smooth);
        Ok(())
    }

    /// Render one frame at `frame_time_s` and read it into `out`.
    ///
    /// `out` must be exactly `frame_bytes()` long. The rows are bottom-up; the
    /// ffmpeg command adds `-vf vflip`.
    pub fn render_frame(&mut self, frame_time_s: f64, out: &mut [u8]) -> Result<()> {
        ensure!(
            frame_time_s.is_finite() && frame_time_s >= 0.0,
            "frame time must be a finite, non-negative number"
        );

        let handle = *self.pm.get_instance().borrow();

        unsafe {
            // Deterministic offline timing. This is why ticket 0018 exists.
            projectm_sys::projectm_set_frame_time(handle, frame_time_s);
            // Render directly into our FBO, not the default framebuffer.
            projectm_sys::projectm_opengl_render_frame_fbo(handle, self.gl.fbo());
        }

        self.gl.read_pixels_rgba(out)?;
        Ok(())
    }

    /// Number of bytes in one RGBA frame.
    pub fn frame_bytes(&self) -> usize {
        self.gl.frame_bytes()
    }

    /// The fps projectM was configured with.
    pub fn fps(&self) -> u32 {
        self.fps
    }
}
```

Register the module:

```rust
#[cfg(feature = "projectm")]
pub mod renderer;
```

### 2. Add an ignored smoke test

`tests/renderer_smoke.rs`:

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
fn renders_a_non_black_frame() {
    let config = config();
    let mut renderer = Renderer::new(&config, Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");

    let mut frame = vec![0_u8; renderer.frame_bytes()];
    renderer
        .render_frame(0.0, &mut frame)
        .expect("frame render failed");

    let non_black = frame
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0)
        .count();

    assert!(
        non_black > 0,
        "renderer returned an all-black frame; check the FBO and preset"
    );
}

#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn rejects_a_negative_frame_time() {
    let config = config();
    let mut renderer = Renderer::new(&config, Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");
    let mut frame = vec![0_u8; renderer.frame_bytes()];

    assert!(renderer.render_frame(-0.1, &mut frame).is_err());
}
```

## Tests to run

Default suite (must pass without native libraries):

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Native compile and ignored tests:

```sh
cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture
```

If EGL surfaceless fails, prefix the last command with `xvfb-run -a`.

## Acceptance criteria

- [x] `Renderer::new` creates the GL context before projectM.
- [x] A NULL projectM handle returns an error, not a panic.
- [x] `set_window_size`, `set_fps`, and `set_beat_sensitivity` are applied.
- [x] `set_preset_locked(true)` prevents projectM from switching on its own.
- [x] `render_frame` sets a fixed frame time and renders into the FBO.
- [x] The ignored smoke test reads back at least one non-black pixel.
- [x] Negative or non-finite frame times are rejected.

## Notes for the implementer

- `ProjectM::create()` returns a wrapper even if the underlying pointer is
  NULL. Always check `get_instance()`.
- Do not call `pm.render_frame()` here. The explicit
  `projectm_opengl_render_frame_fbo` call is what keeps projectM off the
  default framebuffer.
- Do not flip the image in Rust. The ffmpeg argument builder adds `-vf vflip`.
- If `set_preset_locked(true)` causes the first manual preset load to fail,
  check the projectM version. Manual loads should still work; record the
  behaviour in `docs/spike-projectm.md`.
- If the smoke test is black, try loading the preset before
  `set_preset_locked(true)` and compare. Do not change the production path
  without recording why.
