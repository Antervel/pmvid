# pmvid implementation tickets

These tickets break the PRD in `sdlc/PRD.md` into small, sequential units of work.
They are written for a local coding model: each ticket names the files to touch,
shows the intended shape of the code, and lists the exact checks to run.

Work them **in order** unless a ticket says otherwise. Do not start a ticket until
its dependencies are green.

---

## Global rules (apply to every ticket)

1. **KISS.** Write the smallest code that satisfies the ticket. No speculative
   abstractions, no plugin systems, no extra configuration unless the ticket asks
   for it. If a function is only used once, keep it simple.
2. **Every behaviour change needs a test.** A ticket is not done until its tests
   pass. Prefer small unit tests over large integration tests.
3. **Run the checks before declaring a ticket done:**
   ```sh
   cargo fmt --all
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets
   ```
   For native/projectM tickets also run:
   ```sh
   cargo clippy --all-targets --features projectm -- -D warnings
   cargo test --all-targets --features projectm
   ```
4. **No `unwrap()` / `expect()` in library code for user input.** Use `anyhow`
   with a useful context message. `unwrap()` is fine in tests and in examples
   where a panic is an acceptable smoke-test failure.
5. **Keep user-facing errors actionable.** Say what failed and what to do, e.g.
   `"failed to spawn ffmpeg at 'ffmpeg': install ffmpeg or pass --ffmpeg <PATH>"`.
6. **Do not add a dependency without a reason.** Add it in the ticket that needs
   it, pinned to a major version.
7. **Keep modules small and single-purpose.** If a file grows past roughly 300
   lines, split it in the same ticket or a follow-up ticket.
8. **Update this file** when a ticket is finished: change `[ ]` to `[x]` in the
   ticket table and add a one-line note if the implementation differed from the
   plan.
9. **Never delete or overwrite unrelated user work.** Check `git status` first if
   the project is a git repository.

---

## Build features

The native projectM/EGL code is behind a Cargo feature so that the fast,
pure-Rust tests pass on a machine without projectM, EGL, or a GPU:

```toml
[features]
default = []
projectm = [
    "dep:projectm",
    "dep:projectm-sys",
    "dep:khronos-egl",
    "dep:gl",
]
```

Consequences:

- `cargo test` (default features) must always pass without native libraries.
- `cargo build --features projectm` builds the real binary.
- The `pmvid` binary declares `required-features = ["projectm"]`, so
  `cargo run` needs `--features projectm`.
- Native tests that need a display/GPU are marked `#[ignore]` and run manually:
  ```sh
  cargo test --features projectm -- --ignored --nocapture
  ```

This is deliberate: it lets the small coding model make progress on audio,
timebase, presets, CLI, and ffmpeg logic even when projectM is not installed.
Ticket `0018` is the gate for the native dependency; do not skip it.

---

## Standard commands

```sh
# Fast loop (no native dependencies)
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

# Native loop (after ticket 0018 works)
cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm

# Run the tool
cargo run --features projectm -- -i song.mp3 -o song.mp4 --presets ./presets

# Headless software rendering
LIBGL_ALWAYS_SOFTWARE=1 cargo run --features projectm -- -i song.mp3 -o song.mp4
# or, if EGL surfaceless is unavailable:
xvfb-run -a cargo run --features projectm -- -i song.mp3 -o song.mp4
```

---

## Milestone map

| PRD milestone | Tickets |
|---|---|
| Setup | 0001–0002 |
| M1 — audio/timebase/presets/encoder core | 0003–0016 |
| M0 — native spike (GL + projectM) | 0017–0018a |
| M1 — renderer and first MP4 | 0019–0021 |
| M2 — presets, progress, CLI polish | 0022, 0015, 0013–0016 |
| M3 — hardening, determinism, docs | 0023–0025 |
| M4 — headless/Docker, extra formats, batch | 0026–0028 |
| Acceptance | 0029–0030 |

---

## Ticket table

### Setup

- [x] [0001 — Cargo scaffold and feature flags](0001-cargo-scaffold.md) — differed from plan: no repository field; projectm-sys resolved to 1.2.2; projectm build fails until 0018a.
- [ ] [0002 — Formatting, linting, and check script](0002-tooling.md)

### Pure-Rust core (no native libraries)

- [ ] [0003 — Frame/sample timebase math](0003-timebase.md)
- [ ] [0004 — Decode audio to interleaved f32](0004-audio-decode.md)
- [ ] [0005 — Convert decoded audio to stereo](0005-audio-stereo.md)
- [ ] [0006 — Resample to 44.1 kHz](0006-audio-resample.md)
- [ ] [0007 — Audio pipeline and MP3/WAV tests](0007-audio-pipeline.md)
- [ ] [0008 — Scan preset directory](0008-preset-scan.md)
- [ ] [0009 — Seeded preset ordering](0009-preset-order.md)
- [ ] [0010 — Timed preset schedule](0010-preset-schedule.md)
- [ ] [0011 — ffmpeg argument builder](0011-ffmpeg-args.md)
- [ ] [0012 — ffmpeg encoder process](0012-encoder-process.md)
- [ ] [0013 — CLI parsing with clap](0013-cli-parse.md)
- [ ] [0014 — CLI validation](0014-cli-validate.md)
- [ ] [0015 — Progress reporting](0015-progress.md)
- [ ] [0016 — Render config and seed resolution](0016-render-config.md)

### Native spike and renderer

- [ ] [0017 — Headless EGL + FBO smoke test](0017-gl-smoke.md)
- [ ] [0018 — projectM build/API spike](0018-projectm-spike.md)
- [ ] [0018a — Pin and vendor the projectM native build](0018a-projectm-pin.md)
- [ ] [0019 — Renderer initialization and one frame](0019-renderer-init.md)
- [ ] [0020 — Feed PCM and set frame time](0020-audio-feed.md)
- [ ] [0021 — Render loop and MP4 output](0021-render-loop.md)
- [ ] [0022 — Preset switching and failed-preset handling](0022-preset-switching.md)

### Hardening and acceptance

- [ ] [0023 — Error handling and exit codes](0023-errors.md)
- [ ] [0024 — Determinism check](0024-determinism.md)
- [ ] [0025 — README, install docs, licenses](0025-docs.md)
- [ ] [0026 — Dockerfile and headless CI](0026-docker.md)
- [ ] [0027 — Extra audio formats](0027-extra-formats.md)
- [ ] [0028 — Batch mode (P2, optional)](0028-batch.md)
- [ ] [0029 — Sync/acceptance test](0029-acceptance.md)
- [ ] [0030 — Release polish](0030-release.md)

---

## Dependency graph

```
0001 → 0002
0001 → 0003
0001 → 0004 → 0005 → 0006 → 0007
0001 → 0008 → 0009 → 0010
0001 → 0011 → 0012
0001 → 0013 → 0014 → 0016
0001 → 0015
0001 → 0017 → 0018 → 0018a → 0019 → 0020 → 0021
0021 → 0022
0021 → 0023
0021 → 0024
0021 → 0025 → 0026
0021 → 0028
0021 → 0029
0007 → 0027
all  → 0030
```

---

## Definition of done (every ticket)

- [ ] The code compiles with the ticket's feature set.
- [ ] New behaviour has a test (unit test preferred).
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
- [ ] `cargo test --all-targets` passes.
- [ ] If native code changed: the `--features projectm` variants pass, or the
      ticket explains why the native check was run manually.
- [ ] Public functions have a short doc comment saying what they do and what
      they assume (e.g. "`pcm` must be interleaved stereo f32").
- [ ] The ticket file's acceptance checklist is ticked.

---

## Environment notes

- The project targets Linux x86_64 first. The development machine may be
  aarch64; the code must not assume x86.
- `ffmpeg` must be on `PATH` for end-to-end tickets. Check with
  `ffmpeg -version`.
- Building `projectm-sys` needs `cmake`, a C++ toolchain, `clang`/`libclang`
  (for bindgen), `pkg-config`, OpenGL/EGL development headers, and network
  access (it downloads projectM from GitHub). Ticket `0018a` pins the projectM
  commit and vendors patched `projectm` and `projectm-sys` crates under
  `vendor/`.
- Headless rendering needs Mesa EGL and the Mesa DRI driver
  (`libgl1-mesa-dri`). `LIBGL_ALWAYS_SOFTWARE=1` forces llvmpipe.
- Presets are **not** bundled. Use a MilkDrop preset pack or the tiny test
  preset created in ticket `0018`.
