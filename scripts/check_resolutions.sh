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
        -f "${FPS:-30}" \
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
