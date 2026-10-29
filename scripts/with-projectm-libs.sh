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
