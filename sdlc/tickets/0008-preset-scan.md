# 0008 — Scan preset directory

- **Milestone:** M1
- **Depends on:** 0001
- **Files:** `src/presets.rs`, `src/lib.rs`
- **Feature:** default

## Goal

Find all `.milk` preset files under a directory, recursively, and return them in
a deterministic order (sorted by path). Fail with a clear error if the
directory is missing or contains no presets.

## Rules

- KISS: recursive `read_dir`, no glob crate.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Create `src/presets.rs`:

```rust
//! Preset discovery, ordering, and timed switching.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// Recursively find all `.milk` files under `dir`, sorted by path.
///
/// Returns an error if `dir` is not a directory or contains no presets.
pub fn scan(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        bail!(
            "preset directory '{}' does not exist or is not a directory",
            dir.display()
        );
    }

    let mut presets = Vec::new();
    walk(dir, &mut presets)
        .with_context(|| format!("scanning preset directory '{}'", dir.display()))?;
    presets.sort();

    if presets.is_empty() {
        bail!(
            "no .milk presets found in '{}'; pass --presets <DIR>",
            dir.display()
        );
    }

    Ok(presets)
}

fn walk(dir: &Path, presets: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            walk(&path, presets)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("milk"))
        {
            presets.push(path);
        }
    }
    Ok(())
}
```

`is_some_and` requires Rust 1.70; the crate's MSRV is 1.75, so it is fine.
If the local toolchain is older, use `.map_or(false, |ext| ...)`.

Register the module:

```rust
pub mod presets;
```

## Tests

Add to `src/presets.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"[preset00]\n").unwrap();
    }

    #[test]
    fn finds_milk_files_recursively_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("b.milk"));
        touch(&root.join("a.milk"));
        touch(&root.join("nested/c.milk"));
        touch(&root.join("nested/ignore.txt"));

        let found = scan(root).unwrap();
        let names: Vec<_> = found
            .iter()
            .map(|path| path.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, vec!["a.milk", "b.milk", "nested/c.milk"]);
    }

    #[test]
    fn extension_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("upper.MILK"));

        let found = scan(dir.path()).unwrap();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn missing_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");

        let err = scan(&missing).unwrap_err();
        assert!(format!("{err:#}").contains("does not exist"));
    }

    #[test]
    fn empty_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();

        let err = scan(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("no .milk presets found"));
    }

    #[test]
    fn files_that_are_not_presets_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("readme.txt"));
        touch(&dir.path().join("preset.milk"));

        let found = scan(dir.path()).unwrap();
        assert_eq!(found.len(), 1);
    }
}
```

`tempfile` is already a dev-dependency.

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets presets
cargo test --all-targets
```

## Acceptance criteria

- [x] Nested `.milk` files are found.
- [x] Non-`.milk` files are ignored.
- [x] The result is sorted, so repeated scans are deterministic.
- [x] Missing directory and empty directory return actionable errors.
- [x] Uppercase `.MILK` is accepted.

## Notes for the implementer

- Do not follow directory symlinks. `entry.file_type()` reports a symlink as a
  symlink, so the current code skips it. That is deliberate and prevents loops.
- Do not validate preset contents here. projectM does that; ticket `0022`
  handles failed presets.
- Keep the returned paths absolute if the input directory is absolute. The
  function does not canonicalize; callers pass whatever path they want.
