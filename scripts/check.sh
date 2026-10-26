#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> cargo fmt"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy --all-targets -- -D warnings

echo "==> cargo test"
cargo test --all-targets

echo "==> all default-feature checks passed"
