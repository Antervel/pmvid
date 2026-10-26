# 0024 — Determinism check

- **Milestone:** M3
- **Depends on:** 0021, 0009, 0019
- **Files:** `tests/determinism.rs`, `scripts/check_determinism.sh`,
  `README.md`
- **Feature:** `projectm`

## Goal

Prove that the same input + seed + frame timing produces the same visual
output across two runs. This depends on two earlier decisions:

- the preset order is seeded (`0009`),
- projectM uses our frame time, not the wall clock (`0019`/`0020`).

## Rules

- KISS: render twice, decode both MP4s to raw frames, compare bytes.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below.

## Implementation

### 1. Add an ignored determinism test

`tests/determinism.rs`:

```rust
#![cfg(feature = "projectm")]

use std::path::{Path, PathBuf};
use std::process::Command;

use pmvid::app::run_with_config;
use pmvid::config::RenderConfig;

fn make_wav(path: &Path) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for frame in 0..22_050 {
        let value = (frame as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44_100.0).sin();
        let sample = (value * 0.5 * i16::MAX as f32) as i16;
        writer.write_sample(sample).unwrap();
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();
}

fn config(input: PathBuf, output: PathBuf, seed: u64) -> RenderConfig {
    RenderConfig {
        input,
        output,
        width: 32,
        height: 32,
        fps: 30,
        presets: PathBuf::from("tests/fixtures"),
        preset_duration: 5.0,
        beat_sensitivity: 1.0,
        seed,
        crf: 23,
        ffmpeg: PathBuf::from("ffmpeg"),
    }
}

fn decoded_rgba(path: &Path) -> Vec<u8> {
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .expect("ffmpeg must be installed for the determinism test");
    assert!(
        output.status.success(),
        "ffmpeg failed to decode {}",
        path.display()
    );
    output.stdout
}

#[test]
#[ignore = "requires EGL/GL, projectM, ffmpeg, and the native projectm feature"]
fn same_seed_produces_identical_frames() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("tone.wav");
    make_wav(&input);

    let a = dir.path().join("a.mp4");
    let b = dir.path().join("b.mp4");

    run_with_config(config(input.clone(), a.clone(), 42)).unwrap();
    run_with_config(config(input, b.clone(), 42)).unwrap();

    let frames_a = decoded_rgba(&a);
    let frames_b = decoded_rgba(&b);

    assert_eq!(
        frames_a.len(),
        frames_b.len(),
        "the two renders have different frame counts"
    );
    assert_eq!(frames_a, frames_b, "decoded frames differ");
}
```

If the test fails with identical frame counts but a few differing bytes, first
check that projectM is using the fixed frame time. A one-frame timing difference
will change the visuals.

### 2. Add a manual script

`scripts/check_determinism.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

input="${1:?usage: check_determinism.sh <audio-file> [preset-dir]}"
presets="${2:-tests/fixtures}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for run in a b; do
    cargo run --release --features projectm -- \
        -i "$input" \
        -o "$tmp/$run.mp4" \
        --presets "$presets" \
        --seed 42 \
        --preset-duration 5 \
        --crf 20
    ffmpeg -v error -i "$tmp/$run.mp4" -f rawvideo -pix_fmt rgba - \
        > "$tmp/$run.rgba"
done

sha256sum "$tmp/a.rgba" "$tmp/b.rgba"
cmp "$tmp/a.rgba" "$tmp/b.rgba"
echo "deterministic: decoded frames match"
```

Make it executable:

```sh
chmod +x scripts/check_determinism.sh
```

### 3. Document the seed

Add to `README.md`:

````markdown
## Reproducible renders

Pass `--seed <N>` to make the preset order deterministic. The seed is printed
at the start of every render, including when it was chosen randomly.

```sh
pmvid -i song.mp3 -o song.mp4 --seed 42
```
````

## Tests to run

Default suite:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Native checks:

```sh
cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture
```

Manual:

```sh
./scripts/check_determinism.sh /path/to/short.mp3 tests/fixtures
```

## Acceptance criteria

- [ ] Two renders with the same seed produce identical decoded frames.
- [ ] The ignored determinism test passes manually.
- [ ] The manual script prints matching SHA-256 hashes and `cmp` succeeds.
- [ ] The README explains `--seed`.
- [ ] The seed is visible in the render output.

## Notes for the implementer

- Compare **decoded** frames, not MP4 bytes. MP4 metadata can include
  timestamps that differ between runs.
- Do not compare only the first frame; the whole point is that the animation
  evolves identically.
- If x264 threading makes decoded frames differ by a few bytes, check whether
  the difference is only encoder noise by comparing a short clip at `--crf 0`.
  Do not weaken the test to "looks similar" without recording why.
- The test uses a WAV input to avoid MP3 encoder delay differences. The
  production path still supports MP3.
