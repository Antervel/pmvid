# pmvid

Render a [projectM](https://github.com/projectM-visualizer/projectm)
(MilkDrop-compatible) visualization for an audio file into an MP4.

`pmvid` is an offline renderer: it decodes the audio up front, feeds
per-frame PCM to projectM, renders into a headless OpenGL framebuffer, and
pipes raw frames to ffmpeg. Output does not depend on how fast the machine
renders.

## Status

Early v0.1. Linux x86_64 is the primary target. Windows/macOS are untested.

## Requirements

- Rust stable (1.75 or newer)
- CMake, a C++ toolchain, `git`, `pkg-config`
- `clang` and `libclang-dev` (for `projectm-sys`/bindgen)
- OpenGL and EGL development headers (Mesa)
- `ffmpeg` on `PATH` (or pass `--ffmpeg <PATH>`)
- A directory of MilkDrop `.milk` presets

Debian/Ubuntu:

```sh
sudo apt update
sudo apt install build-essential cmake git clang libclang-dev pkg-config \
  libegl1-mesa-dev libgl1-mesa-dev libgl1-mesa-dri ffmpeg
```

`projectm-sys` builds projectM from source on the first build. It needs network
access to clone the projectM repository. Expect the first build to take several
minutes. The pinned build does not need GLM or SDL2; the vendored
`projectm-sys` builds projectM without a UI.

The pinned projectM release is 4.2.0; offline frame timing requires projectM
4.2 or newer.

## Build

The native renderer is behind the `projectm` feature:

```sh
cargo build --release --features projectm
```

Run the fast, pure-Rust tests without native libraries:

```sh
cargo test
```

Run the native checks:

```sh
cargo test --features projectm
```

## Install

Build and install into `~/.local`:

```sh
PREFIX="$HOME/.local" ./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
pmvid --version
```

The script copies the pinned projectM shared libraries next to the binary and sets an rpath, so no `LD_LIBRARY_PATH` is needed after installation.

For development, run through the wrapper:

```sh
./scripts/with-projectm-libs.sh cargo run --features projectm -- \
  -i song.mp3 -o song.mp4 --presets ./presets
```

## Docker image

CI publishes the image to GHCR when the default branch or a `v*` tag is pushed.
Pull requests build and smoke-test the image but do not publish it.

```sh
docker pull ghcr.io/antervel/pmvid:latest
docker run --rm -v "$PWD:/work" ghcr.io/antervel/pmvid:latest \
  -i song.mp3 -o song.mp4 --presets /work/presets
```

The preset directory must be mounted too; presets are not bundled. The image
runs headless with Mesa software rendering (`LIBGL_ALWAYS_SOFTWARE=1`).

The GHCR package is private by default. Either set its visibility to public in
the package settings or run `docker login ghcr.io` with a token that can read
packages before pulling.

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

## Usage

```sh
pmvid -i song.mp3 -o song.mp4 --presets ./presets
```

Supported input formats: MP3, WAV, FLAC, OGG/Vorbis, AAC/M4A.

All options:

```text
Render projectM visualizations for an audio file into an MP4

Usage: pmvid [OPTIONS] --input <FILE> --output <FILE>

Options:
  -i, --input <FILE>           Input audio file
  -o, --output <FILE>          Output video file
  -W, --width <PX>             Output width in pixels [default: 1280]
  -H, --height <PX>            Output height in pixels [default: 720]
  -f, --fps <N>                Output frames per second [default: 30]
  -p, --presets <DIR>          Directory containing .milk presets [default: ./presets]
      --preset-duration <SEC>  Seconds to show each preset [default: 20]
      --beat-sensitivity <F>   projectM beat sensitivity [default: 1]
      --seed <N>               RNG seed for the preset order. Omit for a random seed
      --crf <N>                x264 CRF quality (lower is better, 0-51) [default: 20]
      --ffmpeg <PATH>          ffmpeg binary to use [default: ffmpeg]
  -h, --help                   Print help
  -V, --version                Print version
```

Output is H.264 video with AAC audio.

## Batch mode

```sh
./scripts/batch.sh ./music ./videos --presets ./presets --seed 42
```

This renders every `.mp3`, `.flac`, `.wav`, `.ogg`, and `.m4a` in `./music`
to `./videos/<name>.mp4`.

The script loops over the supported audio files in the input directory (shallow
scan only; it does not recurse into subdirectories) and calls the single-file
CLI once per file, passing any extra arguments through so `--presets`,
`--seed`, and quality flags keep working. It creates the output directory if
it does not exist and skips non-audio files.

Files that share a stem but differ in extension (for example `song.mp3` and
`song.flac`) collide; the later render overwrites the earlier one.

## Presets

Presets are not bundled. Download a MilkDrop preset pack like [Milkdrop "Cream 
of the Crop" Presets](https://github.com/projectM-visualizer/presets-cream-of-the-crop) 
and point `--presets` at the directory. `pmvid` scans recursively for `.milk` files.

A tiny test preset lives in `tests/fixtures/simple.milk`; it is not a
substitute for a real preset pack.

## Headless rendering

`pmvid` uses EGL and does not create a window. On a server without a GPU,
force Mesa software rendering:

```sh
LIBGL_ALWAYS_SOFTWARE=1 pmvid -i song.mp3 -o song.mp4 --presets ./presets
```

If EGL surfaceless is unavailable, run under Xvfb:

```sh
xvfb-run -a pmvid -i song.mp3 -o song.mp4 --presets ./presets
```

## Reproducible renders

Pass `--seed <N>` to make the preset order deterministic. The seed is printed
at the start of every render, including when it was chosen randomly.

```sh
pmvid -i song.mp3 -o song.mp4 --seed 42
```

## Troubleshooting

| Symptom | Fix |
|---|---|
| `failed to spawn ffmpeg` | Install ffmpeg or pass `--ffmpeg /path/to/ffmpeg`. |
| `no .milk presets found` | Point `--presets` at a directory containing `.milk` files. |
| `loading libEGL.so.1` fails | Install `libegl1-mesa-dev` (build) and `libegl1` (runtime). |
| EGL works but rendering fails on a server | Install `libgl1-mesa-dri` for Mesa's software rasterizer. |
| EGL display/context fails | Try `LIBGL_ALWAYS_SOFTWARE=1` or `xvfb-run -a`. |
| `libprojectM-4.so: cannot open shared object file` | Use `./scripts/install.sh`, or for a raw `cargo run` use `./scripts/with-projectm-libs.sh`. |
| `unsupported or corrupt audio file` | Check the file with `ffprobe`; supported formats are MP3, WAV, FLAC, OGG/Vorbis, and AAC/M4A. |
| Build fails in projectM CMake | Install CMake and a full C++ toolchain; check the first build has network access. |

### Runtime library path

The pinned `projectm-sys` links shared `libprojectM-4` libraries from its build
directory but does not install an rpath.

The installed binary gets an rpath and does not need `LD_LIBRARY_PATH`. For a
raw local run, use the development wrapper:

```sh
./scripts/with-projectm-libs.sh cargo run --features projectm -- \
  -i song.mp3 -o song.mp4 --presets ./presets
```

## Licensing

`pmvid` is dual-licensed under MIT or Apache-2.0. See `LICENSE-MIT` and
`LICENSE-APACHE`.

`pmvid` links against projectM, which is LGPL-2.1-or-later. projectM source is
downloaded at build time from a pinned commit and is not included in this
repository. The `projectm` and `projectm-sys` wrapper crates are vendored under
`vendor/` and are also LGPL-2.1-or-later. Presets are not bundled and have
their own licenses; check the preset pack's license before redistributing
output or presets.

See `NOTICE` for details.

## Development

```sh
./scripts/check.sh          # fmt + clippy + tests (default features)
cargo test --features projectm
```

The implementation tickets live in `sdlc/tickets/`.
