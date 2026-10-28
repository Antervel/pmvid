# 0028 — Batch mode (P2, optional)

- **Milestone:** M4
- **Depends on:** 0021, 0025
- **Files:** `scripts/batch.sh`, `tests/batch_script.rs`, `README.md`
- **Feature:** none (shell script) / default for the Rust test

## Goal

Render every audio file in a directory to the corresponding MP4 in an output
directory, without changing the stable single-file CLI.

Do this as a small shell script rather than a new clap subcommand. The PRD
marks batch mode P2; a script is enough and avoids a CLI refactor.

**Do not start this ticket until the single-file renderer works.** If you are
blocked on projectM, skip it.

## Rules

- KISS: one loop, one output name per input file.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

### 1. `scripts/batch.sh`

```bash
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
    base="$(basename "$file")"
    base="${base%.*}"
    output="$output_dir/$base.mp4"
    echo "rendering '$file' -> '$output'"
    "$pmvid_bin" -i "$file" -o "$output" "$@"
done
```

Make it executable:

```sh
chmod +x scripts/batch.sh
```

Notes:

- `PMVID_BIN` is only for testing. Normal users do not need it.
- The script does not overwrite-check; `pmvid` already passes `-y` to ffmpeg.
- Files with the same stem but different extensions would collide. That is
  acceptable for v1; document it.

### 2. Add a test

`tests/batch_script.rs`:

```rust
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn fake_pmvid(dir: &Path, log: &Path) -> std::path::PathBuf {
    let path = dir.join("fake-pmvid.sh");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\n",
        log.display()
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn renders_each_supported_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.mp3"), b"fake").unwrap();
    fs::write(input.join("b.wav"), b"fake").unwrap();
    fs::write(input.join("notes.txt"), b"ignore me").unwrap();

    let log = dir.path().join("calls.log");
    let fake = fake_pmvid(dir.path(), &log);

    let status = Command::new("bash")
        .arg("scripts/batch.sh")
        .arg(&input)
        .arg(&output)
        .env("PMVID_BIN", fake)
        .status()
        .unwrap();
    assert!(status.success());

    let calls = fs::read_to_string(&log).unwrap();
    assert_eq!(calls.matches("-i").count(), 2);
    assert!(calls.contains("a.mp3"));
    assert!(calls.contains("b.wav"));
    assert!(!calls.contains("notes.txt"));
}

#[test]
fn missing_input_directory_fails() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("calls.log");
    let fake = fake_pmvid(dir.path(), &log);

    let status = Command::new("bash")
        .arg("scripts/batch.sh")
        .arg(dir.path().join("missing"))
        .arg(dir.path().join("output"))
        .env("PMVID_BIN", fake)
        .status()
        .unwrap();

    assert!(!status.success());
}

#[test]
fn empty_input_directory_fails() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    fs::create_dir_all(&input).unwrap();
    let log = dir.path().join("calls.log");
    let fake = fake_pmvid(dir.path(), &log);

    let status = Command::new("bash")
        .arg("scripts/batch.sh")
        .arg(&input)
        .arg(dir.path().join("output"))
        .env("PMVID_BIN", fake)
        .status()
        .unwrap();

    assert!(!status.success());
}
```

The fake pmvid only records arguments; it does not create output files. The
test verifies the batch loop, not the renderer.

### 3. Document it in the README

````markdown
## Batch mode

```sh
./scripts/batch.sh ./music ./videos --presets ./presets --seed 42
```

This renders every `.mp3`, `.flac`, `.wav`, `.ogg`, and `.m4a` in `./music`
to `./videos/<name>.mp4`.
````

## Tests to run

```sh
bash -n scripts/batch.sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets batch_script
cargo test --all-targets
```

Manual test with real audio:

```sh
mkdir -p /tmp/batch-in /tmp/batch-out
cp /path/to/short.mp3 /tmp/batch-in/
./scripts/batch.sh /tmp/batch-in /tmp/batch-out --presets tests/fixtures
ls -l /tmp/batch-out
```

## Acceptance criteria

- [x] The script renders one MP4 per supported input file.
- [x] It ignores non-audio files.
- [x] It creates the output directory if needed.
- [x] It fails clearly for a missing input directory or an empty one.
- [x] The single-file CLI is unchanged.
- [x] The test uses a fake `pmvid` and does not need projectM/ffmpeg.

## Notes for the implementer

- Do not add a `--batch` flag to `pmvid` for this ticket. It would force
  `-i`/`-o` to become optional and complicate every existing test.
- Do not recurse into subdirectories. The script is intentionally shallow.
- If two inputs share a stem (`song.mp3` and `song.flac`), the second overwrites
  the first. Document this; do not add collision handling now.
- The script passes all extra arguments through to `pmvid`, so `--presets`,
  `--seed`, and quality flags work.
