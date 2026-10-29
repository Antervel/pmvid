#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
prefix="${PREFIX:-$HOME/.local}"
bin_dir="$prefix/bin"
lib_dir="$prefix/lib/pmvid"
rpath='$ORIGIN/../lib/pmvid'

if [[ "$prefix" == *" "* ]]; then
    echo "error: PREFIX must not contain spaces" >&2
    exit 1
fi

mkdir -p "$bin_dir" "$lib_dir"

echo "==> building pmvid"
RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,$rpath" \
    cargo build --release --locked --features projectm

echo "==> installing binary to $bin_dir"
install -m 755 target/release/pmvid "$bin_dir/pmvid"

echo "==> installing projectM libraries to $lib_dir"
found=0
while IFS= read -r -d '' lib; do
    cp -a "$lib" "$lib_dir/"
    found=1
done < <(find target/release -name 'libprojectM-4*.so*' -print0)

if [[ "$found" -eq 0 ]]; then
    echo "error: no libprojectM-4*.so* under target/release" >&2
    exit 1
fi

echo "installed $bin_dir/pmvid"
echo "add $bin_dir to PATH if it is not already there"
