#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <input-dir> <output-dir> [pmvid options...]" >&2
    exit 2
fi

input_dir="$1"
output_dir="$2"
shift 2

if [[ ! -d "$input_dir" ]]; then
    echo "error: input directory '$input_dir' does not exist" >&2
    exit 1
fi

mkdir -p "$output_dir"

shopt -s nullglob
files=("$input_dir"/*.{mp3,flac,wav,ogg,m4a})

if [[ ${#files[@]} -eq 0 ]]; then
    echo "error: no audio files found in '$input_dir'" >&2
    exit 1
fi

pmvid_bin="${PMVID_BIN:-pmvid}"

for file in "${files[@]}"; do
    [[ -f "$file" ]] || continue
    base="$(basename "$file")"
    base="${base%.*}"
    output="$output_dir/$base.mp4"
    if [[ -e "$output" ]]; then
        echo "error: output '$output' already exists; skipping '$file'" >&2
        continue
    fi
    echo "rendering '$file' -> '$output'"
    "$pmvid_bin" -i "$file" -o "$output" "$@"
done
