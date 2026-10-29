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