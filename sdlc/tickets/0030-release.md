# 0030 — Release polish and install checks

- **Milestone:** release
- **Depends on:** 0018a, 0025, 0026, 0029
- **Files:** `scripts/with-projectm-libs.sh`, `scripts/install.sh`, `README.md`,
  `NOTICE`, `.github/workflows/ci.yml`, `docs/release-checklist.md`
- **Feature:** `projectm` for install checks; default for the fast suite

## Goal

Ship a v0.1.0 build that a user can install and run without manually setting
`LD_LIBRARY_PATH`, and prove the release checks are repeatable.

Ticket `0018a` already pins projectM and vendors the patched `projectm` and
`projectm-sys` crates. This ticket turns that build into a usable install,
updates the licensing notice, and runs the final acceptance pass.

## Rules

- KISS: one install script, one development wrapper, no package manager
  integration.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are manual and may take several minutes.

## Implementation

### 1. Add a development wrapper

`scripts/with-projectm-libs.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

profile="${PROFILE:-debug}"
lib_dir="$(find "target/$profile" -name 'libprojectM-4.so*' -printf '%h\n' | head -n1)"

if [[ -z "$lib_dir" ]]; then
    echo "error: projectM libraries not found under target/$profile" >&2
    echo "run: cargo build --features projectm" >&2
    exit 1
fi

export LD_LIBRARY_PATH="$lib_dir:${LD_LIBRARY_PATH:-}"
exec "$@"
```

Make both scripts executable:

```sh
chmod +x scripts/with-projectm-libs.sh
```

Use it for local development:

```sh
./scripts/with-projectm-libs.sh cargo run --features projectm -- \
  -i song.mp3 -o song.mp4 --presets ./presets
```

Do not put this wrapper in front of the installed binary; the installed binary
gets an rpath instead.

### 2. Add an install script

`scripts/install.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

prefix="${PREFIX:-$HOME/.local}"
bin_dir="$prefix/bin"
lib_dir="$prefix/lib/pmvid"
rpath='$ORIGIN/../lib/pmvid'

if [[ "$prefix" == *" "* ]]; then
    echo "error: PREFIX must not contain spaces" >&2
    exit 1
fi

mkdir -p "$bin_dir" "$lib_dir"

echo "==> building pmvid"
RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,$rpath" \
    cargo build --release --locked --features projectm

echo "==> installing binary to $bin_dir"
install -m 755 target/release/pmvid "$bin_dir/pmvid"

echo "==> installing projectM libraries to $lib_dir"
found=0
while IFS= read -r -d '' lib; do
    cp -a "$lib" "$lib_dir/"
    found=1
done < <(find target/release -name 'libprojectM-4*.so*' -print0)

if [[ "$found" -eq 0 ]]; then
    echo "error: no libprojectM-4*.so* under target/release" >&2
    exit 1
fi

echo "installed $bin_dir/pmvid"
echo "add $bin_dir to PATH if it is not already there"
```

The rpath uses `$ORIGIN`, so the install can be moved as long as `bin/` and
`lib/pmvid/` stay together. Verify it after installing:

```sh
readelf -d "$prefix/bin/pmvid" | grep -E 'RPATH|RUNPATH'
ldd "$prefix/bin/pmvid" | grep projectM
```

`ldd` must resolve `libprojectM-4.so` and `libprojectM-4-playlist.so`; it must
not print "not found".

### 3. Update the README install section

Add or replace these sections. Keep the existing requirements, usage, presets,
and troubleshooting content.

````markdown
## Install

Build and install into `~/.local`:

```sh
PREFIX="$HOME/.local" ./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
pmvid --version
```

The script copies the pinned projectM shared libraries next to the binary and
sets an rpath, so no `LD_LIBRARY_PATH` is needed after installation.

For development, run through the wrapper:

```sh
./scripts/with-projectm-libs.sh cargo run --features projectm -- \
  -i song.mp3 -o song.mp4 --presets ./presets
```

## Reproducible native build

The native dependency is pinned. `vendor/projectm-sys` builds projectM from
commit `349b7cc2db312d47cef6be24c6856ced6643a385` and uses the 4.2 frame-time
and FBO APIs. `vendor/projectm` is the patched Rust wrapper. The projectM source
is downloaded at build time; it is not committed here.

To build offline, point the build at an existing checkout:

```sh
PMVID_PROJECTM_SOURCE_DIR=/path/to/projectm \
  cargo build --release --locked --features projectm
```
````

Update the "Runtime library path" troubleshooting entry to say that the
installed binary should not need it, and that the wrapper is the development
fallback:

````markdown
| `libprojectM-4.so: cannot open shared object file` | Use `./scripts/install.sh`, or for a raw `cargo run` use `./scripts/with-projectm-libs.sh`. |
````

### 4. Update `NOTICE`

Add the vendored wrapper and the actual projectM license:

```text
pmvid
Copyright (c) 2026 pmvid contributors

This product links against projectM
(https://github.com/projectM-visualizer/projectm), which is licensed under the
GNU Lesser General Public License, version 2.1 or later (LGPL-2.1-or-later).
projectM source code is downloaded at build time from a pinned commit and is
not included in this repository.

The vendored crates in vendor/projectm and vendor/projectm-sys are patched
copies of projectm 3.1.2 and projectm-sys 1.0.7. Both are distributed under
LGPL-2.1-or-later. See vendor/projectm/LICENSE and vendor/projectm-sys/LICENSE.

MilkDrop presets are not bundled with pmvid. Preset files have their own
licenses and are the responsibility of the user.

The tiny test preset in tests/fixtures/simple.milk is derived from projectM's
test presets (LGPL-2.1-or-later) and is used only for testing.
```

Do not claim projectM is MIT/Apache. Only `pmvid` itself is dual-licensed.

### 5. Audit CI one last time

Ticket `0026` already made the native job required. Confirm:

- the fast job runs fmt, Clippy, and tests with default features;
- the native job installs the native build dependencies and runs
  `cargo clippy --all-targets --features projectm -- -D warnings`;
- the native job compiles the feature tests with
  `cargo test --all-targets --features projectm --no-run`;
- there is no `continue-on-error: true`;
- `Cargo.lock` is committed.

Do not add a CI job that runs the ignored GL tests unless the runner has Mesa
and the job is allowed to be slow. The manual checks in ticket `0029` cover the
GPU-dependent path.

### 6. Write `docs/release-checklist.md`

Record the exact release pass:

```markdown
# Release checklist

- [ ] `Cargo.toml` version is 0.1.0.
- [ ] `Cargo.lock` is committed.
- [ ] projectM commit is pinned in `vendor/projectm-sys/build.rs`.
- [ ] `cargo tree -i projectm --features projectm` shows `vendor/projectm`.
- [ ] `cargo tree -i projectm-sys --features projectm` shows
      `vendor/projectm-sys`.
- [ ] `LICENSE-MIT`, `LICENSE-APACHE`, and `NOTICE` are present.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
- [ ] `cargo test --all-targets` passes.
- [ ] `cargo clippy --all-targets --features projectm -- -D warnings` passes.
- [ ] `cargo test --all-targets --features projectm` passes.
- [ ] `LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture` passes.
- [ ] `PREFIX=/tmp/pmvid-install ./scripts/install.sh` succeeds.
- [ ] Installed `pmvid --version` runs without `LD_LIBRARY_PATH`.
- [ ] A 2-second MP3 renders to an H.264/AAC MP4 with the installed binary.
- [ ] `docker build -t pmvid .` succeeds.
- [ ] The native CI job is green.
- [ ] Ticket `0029` acceptance checks were reviewed.
```

### 7. Run the final checks

Fast suite:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Native suite:

```sh
cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture
```

If surfaceless EGL fails, use `xvfb-run -a` for the ignored tests.

Install check:

```sh
tmp="$(mktemp -d)"
PREFIX="$tmp" ./scripts/install.sh
"$tmp/bin/pmvid" --version
ldd "$tmp/bin/pmvid" | grep projectM
readelf -d "$tmp/bin/pmvid" | grep -E 'RPATH|RUNPATH'
```

End-to-end check with the installed binary:

```sh
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=2" /tmp/tone.mp3
"$tmp/bin/pmvid" \
  -i /tmp/tone.mp3 \
  -o /tmp/tone.mp4 \
  --presets tests/fixtures \
  --seed 42
ffprobe -v error -show_entries stream=codec_name,width,height \
  -of default=nw=1 /tmp/tone.mp4
```

Expected: H.264 video, AAC audio, and no missing-library error.

Docker check:

```sh
docker build -t pmvid .
docker run --rm pmvid --version
```

## Acceptance criteria

- [ ] `scripts/with-projectm-libs.sh` runs a development binary with the
      projectM libraries on `LD_LIBRARY_PATH`.
- [ ] `scripts/install.sh` installs the binary and both projectM libraries.
- [ ] The installed binary runs without `LD_LIBRARY_PATH`.
- [ ] `readelf` shows an `$ORIGIN`-relative rpath.
- [ ] A short render works with the installed binary.
- [ ] The README documents install, development, pinned build, and
      troubleshooting.
- [ ] `NOTICE` names LGPL-2.1-or-later and both vendored crates
      (`projectm` and `projectm-sys`).
- [ ] The native CI job is required and does not use `continue-on-error`.
- [ ] `docs/release-checklist.md` exists and every item is checked.
- [ ] All default and native test commands pass.

## Notes for the implementer

- Do not use `cargo install --path .` as the primary install path. Cargo does
  not copy the projectM shared libraries, so the installed binary would fail to
  start.
- If the installed binary still cannot find `libprojectM-4.so`, inspect
  `readelf -d`. A common mistake is letting the shell expand `$ORIGIN` to an
  empty string while building. Pass it through a variable as shown.
- If `ldd` shows a path inside `target/`, the rpath was not applied. Rebuild
  with the `RUSTFLAGS` from the install script and check for a stale binary.
- Keep the rpath relative. Do not bake the build machine's `target/` path into a
  release binary.
- The projectM shared libraries are LGPL. Copying them next to the binary is
  fine, but keep the license and attribution in `NOTICE`.
- If the install prefix contains spaces, stop and ask the user to choose a
  different prefix. The simple `RUSTFLAGS` form does not quote paths safely.
- Do not tag or publish a release in this ticket. It produces the checklist and
  a green local/CI build; tagging is a human decision.
