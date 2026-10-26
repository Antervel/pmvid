# 0001 — Cargo scaffold and feature flags

- **Milestone:** setup
- **Depends on:** nothing
- **Files:** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `src/lib.rs`,
  `src/main.rs`, `.gitignore`, `README.md`
- **Feature:** default

## Goal

Create a minimal Rust crate that builds and tests with no native dependencies,
and reserve a `projectm` feature for the GL/projectM code added later.

## Rules

- KISS: only create files that this ticket needs.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

### 1. Create `Cargo.toml`

```toml
[package]
name = "pmvid"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT OR Apache-2.0"
description = "Render projectM visualizations for an audio file into an MP4"
repository = "https://github.com/example/pmvid" # optional; remove if unknown

[lib]
name = "pmvid"
path = "src/lib.rs"

[[bin]]
name = "pmvid"
path = "src/main.rs"
required-features = ["projectm"]

[features]
default = []
projectm = [
    "dep:projectm",
    "dep:projectm-sys",
    "dep:khronos-egl",
    "dep:gl",
]

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
symphonia = { version = "0.5.5", features = ["mp3"] }
rand = "0.8"
indicatif = "0.17"

# Native renderer dependencies. Keep them optional so `cargo test` works
# without projectM/EGL installed.
projectm = { version = "3.1.2", optional = true }
projectm-sys = { version = "1.0.7", optional = true }
khronos-egl = { version = "6", features = ["dynamic"], optional = true }
gl = { version = "0.14", optional = true }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
hound = "3.5"
```

Notes:

- `required-features = ["projectm"]` means `cargo run` needs
  `--features projectm`. This is intentional; see the ticket README.
- Do **not** add `projectm-sys` features manually. The `projectm` crate enables
  the playlist bindings it needs.
- `symphonia` 0.5.x is pinned because its API is stable and widely documented.
  Do not upgrade to 0.6.x in this ticket.

### 2. Create `rust-toolchain.toml`

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

### 3. Create `src/lib.rs`

```rust
//! `pmvid` renders a projectM visualization for an audio file into an MP4.
//!
//! The pure-Rust modules are always available. The native renderer lives behind
//! the `projectm` feature.

/// Sample rate used for projectM analysis.
pub const SAMPLE_RATE: u32 = 44_100;

/// Number of interleaved channels fed to projectM.
pub const CHANNELS: usize = 2;

#[cfg(test)]
mod tests {
    #[test]
    fn constants_are_sane() {
        assert_eq!(super::SAMPLE_RATE, 44_100);
        assert_eq!(super::CHANNELS, 2);
    }
}
```

Do not declare modules that do not exist yet. Later tickets add them.

### 4. Create `src/main.rs`

This binary is only built with `--features projectm`; it will be replaced in
ticket `0021`.

```rust
fn main() {
    eprintln!("pmvid: renderer not implemented yet (see tickets 0017-0021)");
    std::process::exit(1);
}
```

### 5. Create `.gitignore`

```gitignore
/target
*.mp4
*.ppm
*.wav
*.mp3
*.flac
*.ogg
/presets
```

Keep test fixtures that are intentionally committed out of these patterns, or
put them under `tests/fixtures/` and use a negation rule later.

### 6. Create a short `README.md`

````markdown
# pmvid

Render a projectM visualization for an audio file into an MP4.

Status: work in progress. See `sdlc/PRD.md` and `sdlc/tickets/`.

Build the renderer with:

```sh
cargo build --release --features projectm
```
````

### 7. Generate `Cargo.lock`

```sh
cargo generate-lockfile
```

Commit `Cargo.lock` (this is an application, not a library).

## Tests

There is no behaviour yet, so one smoke test is enough.

Run:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Expected: all pass. `cargo test` must **not** try to build projectM.

Also check that the binary is correctly gated:

```sh
cargo build                      # builds only the library, succeeds
cargo build --features projectm  # may fail until ticket 0018; that is OK
```

Do not run `cargo build --features projectm` in CI yet.

## Acceptance criteria

- [ ] `cargo test --all-targets` passes with default features.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `Cargo.toml` has a `projectm` feature and all native deps are optional.
- [ ] `src/lib.rs` exposes `SAMPLE_RATE = 44_100` and `CHANNELS = 2`.
- [ ] No native library is required to run the default test suite.

## Notes for the implementer

- If `cargo` complains about the `repository` field, remove it. It is not
  needed for local work.
- Do not create empty module files "for later". Add them in the ticket that
  implements them. Empty files make it harder to see what is actually done.
- If `cargo generate-lockfile` resolves a newer `projectm-sys` 1.x, that is
  expected. Record the resolved version in the ticket notes. Ticket `0018a`
  replaces both crates with the locally patched copies in `vendor/projectm`
  and `vendor/projectm-sys`.
