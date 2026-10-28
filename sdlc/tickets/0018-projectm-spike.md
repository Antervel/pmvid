# 0018 — projectM build/API spike

- **Milestone:** M0 (spike)
- **Depends on:** 0017
- **Files:** `examples/pm_info.rs`, `Cargo.toml`, `tests/fixtures/simple.milk`,
  `tests/fixtures/README.md`, `docs/spike-projectm.md`
- **Feature:** `projectm`

## Goal

Prove that the pinned `projectm`/`projectm-sys` crates build on this machine,
that a projectM instance can be created with a current EGL context, and that
the two functions we need from projectM 4.2+ are available:

- `projectm_set_frame_time(handle, seconds)` — deterministic offline timing
- `projectm_opengl_render_frame_fbo(handle, fbo_id)` — render into our FBO

If either function is missing, or if the released `projectm` wrapper does not
compile on this machine, do ticket `0018a` (pin and vendor the native build),
then rerun this spike. On aarch64 Linux the released wrapper fails because it
hardcodes `i8` where `std::os::raw::c_char` is `u8`; that is expected and
`0018a` fixes it. Do not silently fall back to wall-clock timing; that breaks
the PRD's determinism goal. Do not start ticket `0019` until this spike passes.

## Rules

- KISS: one example binary, no renderer abstraction yet.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below and must be run manually.

## Prerequisites

`projectm-sys` builds projectM from source. It needs:

```sh
sudo apt install build-essential cmake git clang libclang-dev pkg-config \
  libegl1-mesa-dev libgl1-mesa-dev libgl1-mesa-dri
```

It also needs network access because its `build.rs` clones
`https://github.com/projectM-visualizer/projectm.git` into `OUT_DIR`. The
released `projectm-sys` builds a projectM release (4.0.0 for 1.0.7) that does
**not** contain the 4.2 APIs below. Build the unpatched crate first and record
the exact projectM commit/version in `docs/spike-projectm.md`. If the 4.2
functions are missing, do ticket `0018a` before continuing.

On aarch64 Linux, the released `projectm` wrapper may fail to compile with an
error like `expected *const u8, found *const i8`. That is a known upstream
`c_char` bug, not a problem with your toolchain. Do ticket `0018a` and rerun
this spike.

## Implementation

### 1. Create a tiny test preset

`tests/fixtures/simple.milk`:

```ini
[preset00]
// Minimal test preset derived from projectM's test presets.
// See tests/fixtures/README.md for attribution.
fdecay=0
warp=0
nWaveMode=6
fWaveScale=1
fwavesmoothing=0.01
wave_r=1.0
wave_g=1.0
wave_b=1.0
wave_x=0.500000
wave_y=0.500000
```

`tests/fixtures/README.md`:

```markdown
# Test fixtures

`simple.milk` is a minimal MilkDrop preset derived from the projectM test
presets (`projectm-rs/presets`), which are distributed under
LGPL-2.1-or-later. It is used only by the test suite and examples.

The preset is intentionally trivial: it draws a wave line on a black canvas.
```

### 2. Create `examples/pm_info.rs`

```rust
use anyhow::{ensure, Result};
use pmvid::gl_ctx::GlContext;
use projectm::core::ProjectM;

fn main() -> Result<()> {
    let width = 64;
    let height = 64;

    // The GL context must be current before projectM is created.
    let gl = GlContext::new(width, height)?;

    let pm = ProjectM::create();
    let handle = *pm.get_instance().borrow();
    ensure!(
        !handle.is_null(),
        "projectM_create returned NULL; is the GL context current?"
    );

    println!("projectM version: {}", ProjectM::get_version_string());
    println!("pcm max samples (per channel): {}", ProjectM::pcm_get_max_samples());

    pm.set_window_size(width as usize, height as usize);
    pm.set_fps(30);
    pm.set_beat_sensitivity(1.0);

    let preset = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tests/fixtures/simple.milk".to_string());
    println!("loading preset: {preset}");
    pm.load_preset_file(&preset, false);

    // Available in projectM 4.2+. If this does not compile, read the ticket
    // notes and stop.
    unsafe {
        projectm_sys::projectm_set_frame_time(handle, 0.0);
    }

    gl.bind_fbo();
    // Available in projectM 4.2+. If this does not compile, use
    // `pm.render_frame()` with the FBO bound and record the result.
    unsafe {
        projectm_sys::projectm_opengl_render_frame_fbo(handle, gl.fbo());
    }

    let mut rgba = vec![0_u8; gl.frame_bytes()];
    gl.read_pixels_rgba(&mut rgba)?;

    let non_black = rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0)
        .count();
    println!("non-black pixels: {non_black}/{}", width * height);

    ensure!(
        non_black > 0,
        "projectM rendered an all-black frame; check the FBO binding and preset"
    );

    println!("projectM spike OK");
    Ok(())
}
```

### 3. Add the example to `Cargo.toml`

```toml
[[example]]
name = "pm_info"
path = "examples/pm_info.rs"
required-features = ["projectm"]
```

### 4. Build and run

```sh
cargo build --features projectm --example pm_info
```

This may take several minutes the first time. If it fails, capture the full
error in `docs/spike-projectm.md` and stop. If the error is the aarch64
`*const u8` / `*const i8` mismatch inside the `projectm` wrapper, skip ahead to
ticket `0018a`; that ticket patches the wrapper and reruns this spike.

Check runtime linking:

```sh
ldd target/debug/examples/pm_info | grep -i projectm || true
find target -name 'libprojectM*.so*' -print
```

`projectm-sys` links shared `libprojectM-4` libraries from its `OUT_DIR` but
does not set an rpath. If `ldd` says "not found", run with:

```sh
export LD_LIBRARY_PATH="$(find target -name 'libprojectM-4.so*' -printf '%h\n' | head -n1):${LD_LIBRARY_PATH:-}"
```

Then run:

```sh
LIBGL_ALWAYS_SOFTWARE=1 cargo run --features projectm --example pm_info -- \
  tests/fixtures/simple.milk
```

Expected output includes:

```text
projectM version: 4.2.x
pcm max samples (per channel): 576
loading preset: tests/fixtures/simple.milk
non-black pixels: <number greater than 0>
projectM spike OK
```

### 5. Write `docs/spike-projectm.md`

Record:

- date
- `projectm` crate version and resolved `projectm-sys` version
- projectM version string printed at runtime
- whether `projectm_set_frame_time` and `projectm_opengl_render_frame_fbo`
  compiled
- whether `LD_LIBRARY_PATH` was needed
- EGL backend that worked (surfaceless or xvfb)
- `pcm_get_max_samples()` value
- any build warnings that matter

Use a short table, for example:

```markdown
| Item | Result |
|---|---|
| projectm crate | 3.1.2 |
| projectm-sys | 1.0.7 (patched after 0018a) |
| projectM runtime | 4.2.x |
| set_frame_time | available |
| render_frame_fbo | available |
| runtime lib path | needed LD_LIBRARY_PATH |
| EGL backend | surfaceless Mesa |
| pcm max samples | 576 |
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

cargo clippy --all-targets --features projectm -- -D warnings
cargo build --features projectm --example pm_info
LIBGL_ALWAYS_SOFTWARE=1 cargo run --features projectm --example pm_info -- \
  tests/fixtures/simple.milk
```

If surfaceless EGL fails:

```sh
xvfb-run -a cargo run --features projectm --example pm_info -- \
  tests/fixtures/simple.milk
```

## Acceptance criteria

- [x] `cargo build --features projectm --example pm_info` succeeds.
- [x] The example prints a projectM 4.x version.
- [x] `projectm_set_frame_time` compiles and is called.
- [x] `projectm_opengl_render_frame_fbo` compiles and is called.
- [x] At least one non-black pixel is read back from the test preset.
- [x] Runtime linking is resolved or the `LD_LIBRARY_PATH` workaround is
      documented.
- [x] `docs/spike-projectm.md` records the versions and findings.

## Stop conditions

Stop and report if any of these happen; do not start ticket `0019`:

1. `projectm-sys` cannot build projectM (missing system dependency, CMake
   failure, network failure). If the failure is the stale upstream build script
   rather than a missing system package, do ticket `0018a`.
2. `projectm_set_frame_time` is not in the generated bindings.
3. `projectm_opengl_render_frame_fbo` is not in the generated bindings.
4. The released `projectm` wrapper does not compile because of the hardcoded
   `i8` C-char pointers (common on aarch64 Linux).
5. projectM creates a NULL handle even with a current GL context.
6. The test preset renders all black and changing the FBO binding does not fix
   it.

For (2), (3), and (4), do ticket `0018a`: vendor `projectm` and
`projectm-sys`, patch the `c_char` casts, and pin the build to a projectM commit
that contains the functions. There is no released projectM tag with the 4.2 API
yet, so a commit pin is required.

## Notes for the implementer

- Do not use `pm.render_frame()` in this spike as the primary path. We need the
  explicit FBO call so projectM does not render to the default framebuffer.
- If `projectm-sys` builds a debug library (`libprojectM-4d.so` or
  `libprojectMd.so`), the path is the same; search for `libprojectM*.so*`.
- Do not commit the built projectM tree or `target/`. Only commit the example,
  fixture, and spike notes.
- `projectm` is LGPL. Do not copy its source into this repository. Linking
  against it is fine; note it in `NOTICE` in ticket `0025`.
