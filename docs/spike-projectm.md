# projectM spike (ticket 0018)

Date: 2026-10-28

## Setup

- `projectm` crate: **3.1.2** (now resolved from `vendor/projectm` via `[patch.crates-io]`).
- `projectm-sys` crate: **1.2.2** (declared as `1.0.7` in `Cargo.toml`, semver-resolved to 1.2.2; resolved from `vendor/projectm-sys`).
- projectM runtime: **4.2.0** (version string reported by the library; source pinned to commit `349b7cc2db312d47cef6be24c6856ced6643a385`).

## Findings

- The released `projectm` wrapper (3.1.2) did not compile on this machine (aarch64 Linux): it hardcoded `i8` C pointers while `std::os::raw::c_char` is `u8` on aarch64. The released `projectm-sys` 1.2.2 bundled projectM source v4.1.3, which does not contain the 4.2 API.
- Ticket 0018a therefore vendored `projectm` and `projectm-sys` under `vendor/`, patched the C pointer casts, and pinned the projectM source to commit `349b7cc2db312d47cef6be24c6856ced6643a385` (projectM v4.2.0).

## Spike result

`cargo run --features projectm --example pm_info -- tests/fixtures/simple.milk` with `LIBGL_ALWAYS_SOFTWARE=1`:

```text
EGL 1.5
GL 4.5 (Core Profile) Mesa 25.2.8-0ubuntu0.24.04.2
projectM version: 4.2.0
pcm max samples (per channel): 576
loading preset: tests/fixtures/simple.milk
non-black pixels: 192/4096
projectM spike OK
```

- `projectm_set_frame_time` compiled and was called. ✅
- `projectm_opengl_render_frame_fbo` compiled and was called. ✅
- At least one non-black pixel was read back from the test preset (192/4096). ✅
- A projectM instance was created against a current headless EGL context (surfaceless Mesa, GL 4.5 core).

## Summary

| Item | Result |
|---|---|
| projectm crate | 3.1.2 (vendored) |
| projectm-sys | 1.2.2 (vendored) |
| projectM runtime | 4.2.0 |
| set_frame_time | available |
| render_frame_fbo | available |
| runtime lib path | needed LD_LIBRARY_PATH |
| EGL backend | surfaceless Mesa |
| pcm max samples | 576 |

## Notes

- Runtime linking: the built binary needs `LD_LIBRARY_PATH` pointing at the projectM library output directory, e.g.
  `export LD_LIBRARY_PATH="$(find target -name 'libprojectM-4.so*' -printf '%h\n' | head -n1):${LD_LIBRARY_PATH:-}"`.
- EGL backend: surfaceless (Mesa EGL 1.5) with `LIBGL_ALWAYS_SOFTWARE=1` (Mesa llvmpipe); GL 4.5 core.
- Warnings: none blocking; the native clippy run passes with `-D warnings`.