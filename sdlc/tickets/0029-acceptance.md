# 0029 — Sync and acceptance tests

- **Milestone:** acceptance
- **Depends on:** 0021, 0023, 0024, 0026
- **Files:** `scripts/make_click_track.sh`, `scripts/check_av_sync.sh`,
  `scripts/check_resolutions.sh`, `tests/acceptance.rs`
- **Feature:** `projectm`

## Goal

Verify the PRD's acceptance criteria with repeatable scripts:

- A/V duration drift is under one frame over a long track.
- 720p, 1080p, and a non-16:9 even resolution produce valid output.
- A 4-minute MP3 produces a playable H.264/AAC MP4 with visuals that react to
  the audio.

Some checks are inherently visual. The scripts handle the measurable parts;
the checklist handles the "watch it" part.

## Rules

- KISS: shell scripts plus one ignored Rust test. No video-analysis library.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are manual and may take minutes.

## Implementation

### 1. `scripts/make_click_track.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

output="${1:-click-track.mp3}"
duration="${2:-600}"

ffmpeg -y \
    -f lavfi \
    -i "aevalsrc='if(lt(mod(t\,1)\,0.05)\,0.8*sin(2*PI*1000*t)\,0)':s=44100:d=${duration}" \
    -c:a libmp3lame \
    -q:a 2 \
    "$output"

echo "wrote $output (${duration}s, one click per second)"
```

Make it executable:

```sh
chmod +x scripts/make_click_track.sh
```

### 2. `scripts/check_av_sync.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

input="${1:?usage: check_av_sync.sh <input-audio> <output-mp4> [fps]}"
output="${2:?usage: check_av_sync.sh <input-audio> <output-mp4> [fps]}"
fps="${3:-30}"

pmvid_bin="${PMVID_BIN:-pmvid}"
presets="${PRESETS:-./presets}"

"$pmvid_bin" \
    -i "$input" \
    -o "$output" \
    -f "$fps" \
    --presets "$presets" \
    --seed 42

input_duration="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$input")"
output_duration="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$output")"

echo "input duration:  $input_duration"
echo "output duration: $output_duration"

awk -v a="$input_duration" -v b="$output_duration" -v fps="$fps" '
BEGIN {
    drift = a - b;
    if (drift < 0) drift = -drift;
    limit = 1.0 / fps;
    printf "drift: %.6fs (limit: %.6fs)\n", drift, limit;
    if (drift > limit) {
        print "FAIL: drift exceeds one frame";
        exit 1;
    }
    print "PASS: drift is under one frame";
}'
```

### 3. `scripts/check_resolutions.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

input="${1:?usage: check_resolutions.sh <short-audio-file>}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pmvid_bin="${PMVID_BIN:-pmvid}"
presets="${PRESETS:-./presets}"

for size in 1280x720 1920x1080 854x480; do
    width="${size%x*}"
    height="${size#*x}"
    output="$tmp/$size.mp4"
    echo "rendering $size"
    "$pmvid_bin" \
        -i "$input" \
        -o "$output" \
        -W "$width" \
        -H "$height" \
        -f 30 \
        --presets "$presets" \
        --seed 42

    actual_width="$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "$output")"
    actual_height="$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of csv=p=0 "$output")"
    if [[ "$actual_width" != "$width" || "$actual_height" != "$height" ]]; then
        echo "FAIL: expected ${width}x${height}, got ${actual_width}x${actual_height}"
        exit 1
    fi
    echo "OK: $size"
done

echo "all resolutions passed"
```

### 4. Ignored acceptance test

`tests/acceptance.rs`:

```rust
#![cfg(feature = "projectm")]

use std::path::PathBuf;
use std::process::Command;

use pmvid::app::run_with_config;
use pmvid::config::RenderConfig;

fn make_mp3(path: &std::path::Path, seconds: u32) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=440:duration={seconds}"),
            "-q:a",
            "9",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore = "long-running acceptance test; requires EGL/GL, projectM, ffmpeg"]
fn two_second_render_has_matching_duration_and_codecs() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("tone.mp3");
    let output = dir.path().join("tone.mp4");
    make_mp3(&input, 2);

    let config = RenderConfig {
        input: input.clone(),
        output: output.clone(),
        width: 320,
        height: 180,
        fps: 30,
        presets: PathBuf::from("tests/fixtures"),
        preset_duration: 20.0,
        beat_sensitivity: 1.0,
        seed: 42,
        crf: 28,
        ffmpeg: PathBuf::from("ffmpeg"),
    };

    run_with_config(config).unwrap();

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_name,width,height",
            "-of",
            "default=nw=1",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(probe.status.success());
    let info = String::from_utf8_lossy(&probe.stdout);

    assert!(info.contains("codec_name=h264"), "{info}");
    assert!(info.contains("codec_name=aac"), "{info}");
    assert!(info.contains("width=320"), "{info}");
    assert!(info.contains("height=180"), "{info}");
}
```

## Manual acceptance checklist

### A/V sync

```sh
./scripts/make_click_track.sh /tmp/click-track.mp3 600
./scripts/check_av_sync.sh /tmp/click-track.mp3 /tmp/click-track.mp4 30
```

Then play `/tmp/click-track.mp4` in a player and check:

- the first click and the first visual flash are aligned,
- the click at 5:00 is still aligned,
- the click near 10:00 is still aligned.

A flash is allowed to be one frame late/early. It must not drift progressively.

### Resolutions

```sh
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=2" /tmp/short.mp3
./scripts/check_resolutions.sh /tmp/short.mp3
```

### Real track

```sh
./scripts/check_av_sync.sh /path/to/four-minute-song.mp3 /tmp/song.mp4 30
ffprobe -v error -show_entries format=duration:stream=codec_name,width,height \
  -of default=nw=1 /tmp/song.mp4
```

Watch the output. The visuals should react to the music (not a static frame),
presets should change, and audio should not be ahead or behind.

### Headless

```sh
LIBGL_ALWAYS_SOFTWARE=1 ./scripts/check_av_sync.sh /tmp/short.mp3 /tmp/headless.mp4 30
```

or under Xvfb:

```sh
xvfb-run -a ./scripts/check_av_sync.sh /tmp/short.mp3 /tmp/xvfb.mp4 30
```

## Tests to run

Default suite:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Native ignored acceptance test:

```sh
cargo test --all-targets --features projectm
LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture
```

Scripts:

```sh
bash -n scripts/make_click_track.sh
bash -n scripts/check_av_sync.sh
bash -n scripts/check_resolutions.sh
```

## Acceptance criteria

- [ ] The 10-minute click track renders and its duration drift is under one
      frame.
- [ ] Clicks and visual flashes stay aligned at the start, middle, and end.
- [ ] 1280×720, 1920×1080, and 854×480 outputs have the requested dimensions.
- [ ] A 4-minute MP3 produces a playable H.264/AAC MP4 with reactive visuals.
- [ ] The render works under `LIBGL_ALWAYS_SOFTWARE=1` or `xvfb-run`.
- [ ] The ignored acceptance test passes manually.

## Notes for the implementer

- Do not try to automate "the visuals react to the audio" in this ticket. That
  is a human judgment call; the deterministic frame comparison is ticket
  `0024`.
- 1080p on llvmpipe is slow. Use a 2-second input for the resolution script.
- If duration drift is exactly one frame, check whether ffmpeg's AAC priming
  samples are included in the container duration. A sub-frame difference is
  acceptable; progressive drift is not.
- If clicks drift progressively, the bug is almost always a rounded
  samples-per-frame constant. Re-read ticket `0003` and `0021`.
