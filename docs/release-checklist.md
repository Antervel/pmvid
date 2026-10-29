# Release checklist

- [x] `Cargo.toml` version is 0.1.0.
- [x] `Cargo.lock` is committed.
- [x] projectM commit is pinned in `vendor/projectm-sys/build.rs`.
- [x] `cargo tree -i projectm --features projectm` shows `vendor/projectm`.
- [x] `cargo tree -i projectm-sys --features projectm` shows `vendor/projectm-sys`.
- [x] `LICENSE-MIT`, `LICENSE-APACHE`, and `NOTICE` are present.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --all-targets -- -D warnings` passes.
- [x] `cargo test --all-targets` passes.
- [x] `cargo clippy --all-targets --features projectm -- -D warnings` passes.
- [x] `cargo test --all-targets --features projectm` passes.
- [x] `LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture` passes.
- [x] `PREFIX=/tmp/pmvid-install ./scripts/install.sh` succeeds.
- [x] Installed `pmvid --version` runs without `LD_LIBRARY_PATH`.
- [x] A 2-second MP3 renders to an H.264/AAC MP4 with the installed binary.
- [x] `docker build -t pmvid .` succeeds.
- [x] The native CI job is green.
- [x] Ticket `0029` acceptance checks were reviewed.
- [ ] The CI `docker` job is green and published the release tag.
- [ ] `docker pull ghcr.io/antervel/pmvid:<version>` succeeds.
- [ ] The pulled image runs `--version` and renders a short MP4 headlessly.

### Final checks

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

GHCR check:

```sh
docker pull ghcr.io/antervel/pmvid:latest
docker run --rm ghcr.io/antervel/pmvid:latest --version
```

GHCR headless render check:

```sh
tmp="$(mktemp -d)"
mkdir -p "$tmp/presets"
cp tests/fixtures/simple.milk "$tmp/presets/"
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=2" "$tmp/tone.mp3"
docker run --rm -v "$tmp:/work" ghcr.io/antervel/pmvid:latest \
  -i /work/tone.mp3 -o /work/tone.mp4 --presets /work/presets --seed 42
ffprobe -v error -show_entries stream=codec_name -of csv=p=0 "$tmp/tone.mp4"
```
