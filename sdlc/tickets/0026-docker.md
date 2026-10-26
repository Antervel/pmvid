# 0026 — Dockerfile and headless CI

- **Milestone:** M4
- **Depends on:** 0025
- **Files:** `Dockerfile`, `.dockerignore`, `.github/workflows/ci.yml`
- **Feature:** `projectm` for the image; default for the fast CI job

## Goal

Make headless builds reproducible in a container and keep the fast test suite
running on every push.

The Docker image must contain the projectM shared libraries built by
`projectm-sys`; otherwise the binary fails at runtime with
`libprojectM-4.so: cannot open shared object file`.

## Rules

- KISS: multi-stage Dockerfile, one runtime image.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Docker/CI checks are manual.

## Implementation

### 1. `Dockerfile`

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        cmake \
        git \
        clang \
        libclang-dev \
        pkg-config \
        libegl1-mesa-dev \
        libgl1-mesa-dev \
        ffmpeg \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy manifests first so dependency builds can be cached.
COPY Cargo.toml Cargo.lock ./
COPY vendor ./vendor
COPY src ./src
COPY examples ./examples
COPY tests ./tests
COPY sdlc ./sdlc

RUN cargo build --release --locked --features projectm

# Collect the projectM shared libraries; projectm-sys does not install them.
RUN mkdir -p /out/lib \
    && find target/release -name 'libprojectM-4*.so*' -exec cp -a {} /out/lib/ \; \
    && cp target/release/pmvid /out/pmvid

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        libegl1 \
        libegl-mesa0 \
        libgl1 \
        libgl1-mesa-dri \
        libstdc++6 \
        libgomp1 \
        ffmpeg \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/lib/ /usr/local/lib/
COPY --from=builder /out/pmvid /usr/local/bin/pmvid

RUN ldconfig

# Mesa software rendering works without a GPU or display. The DRI package is
# required: libegl1 only pulls the EGL loader/display, not the rasterizer.
ENV LIBGL_ALWAYS_SOFTWARE=1

WORKDIR /work
ENTRYPOINT ["pmvid"]
```

The pinned `projectm-sys` build sets `ENABLE_SDL_UI=OFF`, so SDL2 and GLM are
not needed. Do not add `libsdl2-dev` or `libglm-dev` back unless the vendored
build changes and the CMake log proves they are required.

### 2. `.dockerignore`

```gitignore
target
.git
.github
*.mp4
*.ppm
presets
```

Do not ignore `tests/fixtures`; the Docker build may compile tests but does not
need to run them. If you want a smaller build context, add
`tests/fixtures/*.mp3` and keep the `.milk` fixture.

### 3. `.github/workflows/ci.yml`

```yaml
name: ci

on:
  push:
  pull_request:

jobs:
  fast:
    name: fmt + clippy + pure-Rust tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --all-targets

  native:
    name: projectM build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \
            build-essential cmake git clang libclang-dev pkg-config \
            libegl1-mesa-dev libgl1-mesa-dev ffmpeg
      - run: cargo clippy --all-targets --features projectm -- -D warnings
      - run: cargo test --all-targets --features projectm --no-run
      - run: cargo build --features projectm
```

The native job is required because ticket `0018a` pins projectM and vendors the
patched `projectm` and `projectm-sys` crates. Do not add `continue-on-error`
back; if the pin breaks, fix the pin deliberately.

## Tests to run

Fast suite:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Docker:

```sh
docker build -t pmvid .
docker run --rm pmvid --version
docker run --rm pmvid --help
```

Render a short file from the host:

```sh
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=2" /tmp/tone.mp3
docker run --rm -v /tmp:/work pmvid \
  -i /work/tone.mp3 -o /work/tone.mp4 --presets /work/presets
ffprobe -v error -show_entries stream=codec_name -of csv=p=0 /tmp/tone.mp4
```

The preset directory must be mounted too. If `/tmp/presets` does not exist,
copy `tests/fixtures` there first.

Check the runtime library:

```sh
docker run --rm --entrypoint sh pmvid -c 'ldd /usr/local/bin/pmvid | grep projectM'
```

Expected: a resolved path under `/usr/local/lib`, not "not found".

## Acceptance criteria

- [ ] `docker build -t pmvid .` succeeds.
- [ ] `docker run --rm pmvid --version` works.
- [ ] `ldd` inside the image resolves `libprojectM`.
- [ ] A short render works inside the container with no display.
- [ ] The fast CI job runs fmt, Clippy, and tests.
- [ ] The native CI job builds the pinned projectM dependency and is required.

## Notes for the implementer

- The first Docker build is slow because projectM is compiled from source.
  Layer ordering (copy `Cargo.toml` first) helps only a little; projectm-sys
  does not cache its CMake build across clean target directories.
- Do not run the renderer as root in production; add a non-root user if you
  deploy this image. It is not required for this ticket.
- Do not bake presets into the image. Mount them at runtime.
- If the runtime image is missing `libprojectM-4-playlist.so`, check that the
  `find` pattern copied both libraries. It uses `libprojectM-4*.so*`.
- If `docker run` fails with "error while loading shared libraries: libGL.so.1",
  install `libgl1` in the runtime stage (already listed).
