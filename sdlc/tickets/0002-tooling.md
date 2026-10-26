# 0002 — Formatting, linting, and check script

- **Milestone:** setup
- **Depends on:** 0001
- **Files:** `rustfmt.toml`, `clippy.toml`, `scripts/check.sh`, `Justfile`
- **Feature:** default

## Goal

Make the quality bar mechanical. Every later ticket ends with the same three
commands, so add them once in a script and document them.

## Rules

- KISS: one script, no custom lint plugins.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

### 1. `rustfmt.toml`

```toml
edition = "2021"
max_width = 100
use_field_init_shorthand = true
use_try_shorthand = true
```

Do not enable unstable options; the project uses stable Rust.

### 2. `clippy.toml`

```toml
msrv = "1.75"
```

Keep Clippy on its default lint groups. The check script adds
`-D warnings`, which is enough. Do not enable `pedantic` globally; it produces
noise that distracts from real bugs.

### 3. `scripts/check.sh`

```bash
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
```

Make it executable:

```sh
chmod +x scripts/check.sh
```

### 4. `Justfile` (optional but useful)

If `just` is installed, add shortcuts. If it is not installed, the script is
still the source of truth.

```make
check:
    ./scripts/check.sh

fmt:
    cargo fmt --all

lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test --all-targets

# Native checks. Only use after ticket 0018.
native-check:
    cargo clippy --all-targets --features projectm -- -D warnings
    cargo test --all-targets --features projectm
```

### 5. Add a native check note

Do **not** put `--features projectm` in `scripts/check.sh` yet. It would make
the script fail on machines without projectM. Add it in ticket `0018` as a
separate `scripts/check-native.sh` if desired.

## Tests

This ticket adds tooling, so the "test" is running the tooling:

```sh
bash -n scripts/check.sh
./scripts/check.sh
```

Expected: the script prints all three sections and exits 0.

Also verify that formatting actually fails on badly formatted code:

```sh
printf 'fn  badly_formatted( ){ }\n' >> src/lib.rs
cargo fmt --all -- --check   # must fail
cargo fmt --all              # fix it
```

Remove the temporary function afterwards.

## Acceptance criteria

- [ ] `rustfmt.toml` and `clippy.toml` exist and use stable options.
- [ ] `scripts/check.sh` is executable and exits 0 on the current tree.
- [ ] The script runs fmt check, Clippy with `-D warnings`, and tests.
- [ ] `Justfile` (if added) calls the same commands.
- [ ] The ticket README's standard commands match the script.

## Notes for the implementer

- `cargo fmt --all -- --check` is the CI-friendly form. `cargo fmt` alone
  rewrites files.
- Do not silence Clippy warnings with `#[allow(...)]` to make this ticket pass.
  Fix the code. If a warning is genuinely wrong, add a narrow `#[allow]` with a
  comment explaining why.
- If `just` is not installed, skip the `Justfile`; it is optional.
