#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
profile="${PROFILE:-debug}"
lib="$(find "$root/target/$profile" -name 'libprojectM-4.so*' -print -quit)"

if [[ -z "$lib" ]]; then
    echo "error: projectM libraries not found under target/$profile" >&2
    echo "run: cargo build --features projectm" >&2
    exit 1
fi
lib_dir="$(dirname "$lib")"

export LD_LIBRARY_PATH="$lib_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$@"
