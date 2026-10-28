# 0018a — Pin and vendor the projectM native build

- **Milestone:** M0 (native dependency)
- **Depends on:** 0017, 0018
- **Files:** `vendor/projectm/**`, `vendor/projectm-sys/**`, `Cargo.toml`,
  `Cargo.lock`, `docs/spike-projectm.md`
- **Feature:** `projectm`

## Goal

Make the native projectM build reproducible, make the 4.2 C API available, and
make the Rust wrapper compile on both x86_64 and aarch64:

- `projectm_set_frame_time`
- `projectm_opengl_render_frame_fbo`

The released `projectm-sys` crate has three problems for this project:

1. it builds a released projectM tag (4.0.0) that lacks the 4.2 API;
2. some versions clone `master`, so two builds can use different source;
3. its generated bindings and Linux link names are stale.

The released `projectm` wrapper crate has a fourth problem: it hardcodes `i8`
for C `char` pointers, but `c_char` is `u8` on aarch64 Linux. The wrapper
therefore fails to compile on the development machine. This ticket vendors and
patches that wrapper too.

Do this ticket if ticket `0018` failed because those functions were missing, or
because `projectm` did not compile on this platform. When it is done, rerun
`0018` and only then start `0019`.

## Rules

- KISS: vendor two small crates, patch one build script and four lines in the
  wrapper. Do not fork the whole dependency tree.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below and must be run manually.
- Do not commit the projectM source checkout or `target/`.

## Why a commit, not a tag?

As of 2026-10-26, projectM's newest release tag is `v4.1.8`. The two functions
above were added for the unreleased 4.2 API. They exist on `master`, so pin a
commit that contains both.

The ticket uses this commit:

```text
349b7cc2db312d47cef6be24c6856ced6643a385
```

Before pinning, verify it still contains both functions:

```sh
curl -fsSL https://raw.githubusercontent.com/projectM-visualizer/projectm/349b7cc2db312d47cef6be24c6856ced6643a385/src/api/include/projectM-4/parameters.h \
  | grep projectm_set_frame_time
curl -fsSL https://raw.githubusercontent.com/projectM-visualizer/projectm/349b7cc2db312d47cef6be24c6856ced6643a385/src/api/include/projectM-4/render_opengl.h \
  | grep projectm_opengl_render_frame_fbo
```

If a later 4.2+ release tag exists, use that tag's commit and update the SHA in
this ticket, `docs/spike-projectm.md`, and the vendor build script.

## Implementation

### 1. Vendor `projectm` 3.1.2 and `projectm-sys` 1.0.7

Download the crate sources rather than copying a `cargo vendor` tree:

```sh
mkdir -p vendor
curl -fsSL https://crates.io/api/v1/crates/projectm-sys/1.0.7/download \
  -o /tmp/projectm-sys-1.0.7.crate
tar -xzf /tmp/projectm-sys-1.0.7.crate -C vendor
mv vendor/projectm-sys-1.0.7 vendor/projectm-sys

curl -fsSL https://crates.io/api/v1/crates/projectm/3.1.2/download \
  -o /tmp/projectm-3.1.2.crate
tar -xzf /tmp/projectm-3.1.2.crate -C vendor
mv vendor/projectm-3.1.2 vendor/projectm
```

Keep both `LICENSE` files. They are LGPL-2.1-or-later; ticket `0030` updates
the root `NOTICE` to mention both vendored copies.

Add `vendor/README.md`:

```markdown
# Vendored dependencies

`projectm-sys` is copied from crates.io 1.0.7 and patched locally:

- the projectM source is pinned to commit
  `349b7cc2db312d47cef6be24c6856ced6643a385` instead of a release tag;
- the CMake install include directory is passed to bindgen;
- Linux links the actual `projectM-4` and `projectM-4-playlist` libraries;
- `PMVID_PROJECTM_SOURCE_DIR` allows an offline build from an existing checkout.

`projectm` is copied from crates.io 3.1.2 and patched locally:

- C `char` pointers use `std::os::raw::c_char` instead of a hardcoded `i8`,
  so the wrapper compiles on aarch64 Linux as well as x86_64.

Do not edit these copies casually. See `sdlc/tickets/0018a-projectm-pin.md`.
```

### 1b. Patch the vendored `projectm` wrapper

The upstream wrapper hardcodes `i8` in four places. On aarch64 Linux,
`std::os::raw::c_char` is `u8`, so the generated bindings and the wrapper
disagree. Change all four to `std::os::raw::c_char`.

In `vendor/projectm/src/core.rs`, make these replacements:

```diff
-                filename.as_ptr() as *mut i8,
+                filename.as_ptr() as *const std::os::raw::c_char,
```

```diff
-            ffi::projectm_load_preset_data(instance, data.as_ptr() as *mut i8, smooth_transition)
+            ffi::projectm_load_preset_data(
+                instance,
+                data.as_ptr() as *const std::os::raw::c_char,
+                smooth_transition,
+            )
```

```diff
-            preset_filename: *const i8,
-            message: *const i8,
+            preset_filename: *const std::os::raw::c_char,
+            message: *const std::os::raw::c_char,
```

Also change the `license` field in `vendor/projectm/Cargo.toml` from the
upstream `LGPL-3.0-or-later` to `LGPL-2.1-or-later`; the bundled `LICENSE` is
LGPL 2.1.

Do not change the wrapper's public API. Tickets `0018`–`0022` call
`ProjectM::create`, `set_window_size`, `set_fps`, `set_beat_sensitivity`,
`set_preset_locked`, `load_preset_file`, `pcm_add_float`,
`pcm_get_max_samples`, and the failure callback.

### 2. Patch the vendored `projectm-sys` `Cargo.toml`

Replace `vendor/projectm-sys/Cargo.toml` with:

```toml
[package]
name = "projectm-sys"
version = "1.0.7"
edition = "2021"
rust-version = "1.65"
license = "LGPL-2.1-or-later"
description = "Bindings for projectM (pmvid pinned build)"
links = "projectm"

[lib]
name = "projectm_sys"
path = "src/lib.rs"

[features]
default = ["playlist"]
playlist = []

[build-dependencies]
bindgen = "0.64.0"
cmake = "0.1.50"
```

The `links` key must stay; it prevents two different `projectm-sys` builds in
one dependency graph.

### 3. Replace `vendor/projectm-sys/build.rs`

```rust
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod build_bindgen;

const PROJECTM_REPO: &str = "https://github.com/projectM-visualizer/projectm.git";
const PROJECTM_COMMIT: &str = "349b7cc2db312d47cef6be24c6856ced6643a385";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PMVID_PROJECTM_SOURCE_DIR");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let source = projectm_source(&out_dir);

    let dst = cmake::Config::new(&source)
        .define("ENABLE_PLAYLIST", "ON")
        .define("ENABLE_INSTALL", "ON")
        .define("ENABLE_DEBUG_POSTFIX", "OFF")
        .define("ENABLE_SDL_UI", "OFF")
        .define("ENABLE_SYSTEM_PROJECTM_EVAL", "OFF")
        .build();

    // GNUInstallDirs may use lib or lib64 depending on the distribution.
    for dir in ["lib", "lib64"] {
        let path = dst.join(dir);
        if path.is_dir() {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    }
    println!("cargo:rustc-link-lib=dylib=projectM-4");
    println!("cargo:rustc-link-lib=dylib=projectM-4-playlist");

    build_bindgen::bindgen(&dst.join("include"));
}

/// Return a projectM source tree, either from the environment or a pinned
/// checkout in `OUT_DIR`.
fn projectm_source(out_dir: &Path) -> PathBuf {
    if let Some(path) = env::var_os("PMVID_PROJECTM_SOURCE_DIR") {
        let path = PathBuf::from(path);
        assert!(
            path.join("CMakeLists.txt").is_file(),
            "PMVID_PROJECTM_SOURCE_DIR must point at a projectM source checkout"
        );
        assert!(
            path.join("vendor/projectm-eval/CMakeLists.txt").is_file(),
            "projectM submodules are missing; run: git -C {} submodule update --init --recursive",
            path.display()
        );
        return path;
    }

    let source = out_dir.join("projectm");
    if source.join("CMakeLists.txt").is_file() {
        if !source.join("vendor/projectm-eval/CMakeLists.txt").is_file() {
            run(
                Command::new("git")
                    .arg("-C")
                    .arg(&source)
                    .args(["submodule", "update", "--init", "--recursive", "--depth=1"]),
            );
        }
        return source;
    }

    // A previous failed clone can leave a partial directory behind.
    if source.exists() {
        fs::remove_dir_all(&source).expect("failed to remove incomplete projectM checkout");
    }

    run(
        Command::new("git")
            .args([
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                PROJECTM_REPO,
            ])
            .arg(&source),
    );
    run(
        Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["fetch", "--depth=1", "origin", PROJECTM_COMMIT]),
    );
    run(
        Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["checkout", "--detach", "FETCH_HEAD"]),
    );
    run(
        Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["submodule", "update", "--init", "--recursive", "--depth=1"]),
    );

    source
}

fn run(command: &mut Command) {
    let status = command
        .status()
        .expect("failed to run git; install git or set PMVID_PROJECTM_SOURCE_DIR");
    assert!(status.success(), "git command failed: {command:?}");
}
```

Notes:

- `ENABLE_DEBUG_POSTFIX=OFF` keeps one library name for debug and release.
  This avoids the `projectM-4d` / `projectM-4` mismatch in upstream 1.0.7.
- `ENABLE_INSTALL=ON` makes CMake install headers into `dst/include`, which
  bindgen needs.
- `ENABLE_SYSTEM_PROJECTM_EVAL=OFF` forces the vendored `projectm-eval`
  submodule. Leaving the upstream default (`ON`) would make the build depend on
  whatever `projectM-Eval` happens to be installed on the machine.
- The `lib`/`lib64` loop handles distributions where `GNUInstallDirs` chooses
  `lib64`. Both paths are only emitted when they exist.
- Do not use `git clone --branch master`; that reintroduces the reproducibility
  bug.
- projectM uses the `vendor/projectm-eval` submodule. The build script runs
  `git submodule update --init --recursive`; if you supply
  `PMVID_PROJECTM_SOURCE_DIR`, initialize its submodules yourself.
- The first build downloads and compiles projectM. It can take several minutes.

### 4. Replace `vendor/projectm-sys/build_bindgen.rs`

```rust
use std::env;
use std::path::{Path, PathBuf};

pub fn bindgen(include_dir: &Path) {
    println!("cargo:rerun-if-changed=bindgen/default.h");
    println!("cargo:rerun-if-changed=bindgen/playlist.h");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let header = if cfg!(feature = "playlist") {
        "bindgen/playlist.h"
    } else {
        "bindgen/default.h"
    };

    let bindings = bindgen::Builder::default()
        .header(header)
        .allowlist_function("projectm_.*")
        .clang_arg(format!("-I{}", include_dir.display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("unable to generate projectM bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("could not write projectM bindings");
}
```

`bindgen/playlist.h` includes both `projectM-4/projectM.h` and
`projectM-4/playlist.h`, so all functions used by the `projectm` wrapper are
generated.

### 5. Point Cargo at the vendored crates

Add to the root `Cargo.toml`:

```toml
[patch.crates-io]
projectm = { path = "vendor/projectm" }
projectm-sys = { path = "vendor/projectm-sys" }
```

Do not change the `projectm` dependency version. It depends on
`projectm-sys` 1.x, and the patches redirect both crates to `vendor/`.

### 6. Rebuild

Force Cargo to rebuild the native crates after changing the wrapper or the
build script:

```sh
cargo clean -p projectm -p projectm-sys
cargo build --features projectm --example pm_info
```

If you need an offline or repeatable source tree:

```sh
git clone https://github.com/projectM-visualizer/projectm.git /tmp/projectm-src
git -C /tmp/projectm-src checkout --detach \
  349b7cc2db312d47cef6be24c6856ced6643a385
git -C /tmp/projectm-src submodule update --init --recursive --depth=1

cargo clean -p projectm -p projectm-sys
PMVID_PROJECTM_SOURCE_DIR=/tmp/projectm-src \
  cargo build --features projectm --example pm_info
```

### 7. Verify the pin and the generated bindings

```sh
# Both crates must come from vendor/, not crates.io.
cargo tree -i projectm --features projectm
cargo tree -i projectm-sys --features projectm

# The wrapper must use c_char, not i8.
grep -n "std::os::raw::c_char" vendor/projectm/src/core.rs

# Find the projectM checkout. If you built with PMVID_PROJECTM_SOURCE_DIR,
# use that path instead.
source_dir="$(find target -path '*/out/projectm/.git' -printf '%h\n' | head -n1)"
if [[ -n "$source_dir" ]]; then
    git -C "$source_dir" rev-parse HEAD
    # expected: 349b7cc2db312d47cef6be24c6856ced6643a385

    grep -R "projectm_set_frame_time" \
      "$source_dir/src/api/include/projectM-4/parameters.h"
    grep -R "projectm_opengl_render_frame_fbo" \
      "$source_dir/src/api/include/projectM-4/render_opengl.h"
fi

ldd target/debug/examples/pm_info | grep -i projectm || true
find target -name 'libprojectM-4*.so*' -print
```

If `ldd` says "not found", that is expected until the runtime path is set:

```sh
lib_dir="$(find target -name 'libprojectM-4.so*' -printf '%h\n' | head -n1)"
LD_LIBRARY_PATH="$lib_dir:${LD_LIBRARY_PATH:-}" \
  LIBGL_ALWAYS_SOFTWARE=1 \
  cargo run --features projectm --example pm_info -- tests/fixtures/simple.milk
```

Record the result and the pinned SHA in `docs/spike-projectm.md`. Ticket `0030`
adds an install script and CI checks for the runtime library path.

## Tests to run

Default suite:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Native build and spike:

```sh
cargo clean -p projectm -p projectm-sys
cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
cargo build --features projectm --example pm_info

lib_dir="$(find target -name 'libprojectM-4.so*' -printf '%h\n' | head -n1)"
LD_LIBRARY_PATH="$lib_dir:${LD_LIBRARY_PATH:-}" \
  LIBGL_ALWAYS_SOFTWARE=1 \
  cargo run --features projectm --example pm_info -- tests/fixtures/simple.milk
```

Expected: `projectM spike OK` and at least one non-black pixel.

If surfaceless EGL fails, prefix the last command with `xvfb-run -a`.

## Acceptance criteria

- [x] `Cargo.toml` contains `[patch.crates-io]` entries for `projectm` and
      `projectm-sys`.
- [x] `cargo tree -i projectm --features projectm` shows the `vendor/` path,
      not a crates.io registry copy.
- [x] `cargo tree -i projectm-sys --features projectm` shows the `vendor/`
      path, not a crates.io registry copy.
- [x] The vendored build uses commit
      `349b7cc2db312d47cef6be24c6856ced6643a385`, not `master` or a tag.
- [x] `PMVID_PROJECTM_SOURCE_DIR` builds without cloning.
- [x] Both 4.2 functions are present in the generated bindings.
- [x] `vendor/projectm/src/core.rs` uses `std::os::raw::c_char`, and the crate
      compiles on aarch64.
- [x] `examples/pm_info.rs` builds and prints `ProjectM spike OK`.
- [x] `vendor/projectm/LICENSE` and `vendor/projectm-sys/LICENSE` are present.
- [x] No projectM source checkout is committed to this repository.

## Notes for the implementer

- If `bindgen` 0.64 fails with the installed Clang, bump it in the vendored
  `Cargo.toml` and use `bindgen::CargoCallbacks::new()` in `build_bindgen.rs`.
  Record the version in `docs/spike-projectm.md`.
- If the linker cannot find `-lprojectM-4`, inspect the CMake output. The
  library may be named `libprojectM-4d` if `ENABLE_DEBUG_POSTFIX` was not
  applied. Do not "fix" it by renaming files in `target/`.
- The `c_char` patch is required on aarch64. On x86_64 it is harmless. If you
  see `expected *const u8, found *const i8`, you are building the unpatched
  wrapper.
- If the commit pin is not reachable with `git fetch --depth=1`, use a full
  clone in `PMVID_PROJECTM_SOURCE_DIR` for local work, but keep the pinned SHA
  in the vendored build script.
- Do not copy projectM's source into this repository. Only the small
  `projectm` and `projectm-sys` wrapper crates are vendored.
- If upstream `projectm-sys` releases a fixed version that pins a 4.2-capable
  commit, you may use it instead. Verify the link names and bindings first, and
  update this ticket's acceptance criteria. The same applies if upstream
  `projectm` fixes the `c_char` casts.
