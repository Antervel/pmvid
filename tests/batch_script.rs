#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn fake_pmvid(dir: &Path, log: &Path) -> std::path::PathBuf {
    let path = dir.join("fake-pmvid.sh");
    let script = format!("#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\n", log.display());
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
    let args: Vec<&str> = calls
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    // Two renders, each: -i <input> -o <output>.
    assert_eq!(args.len(), 8);
    assert_eq!(args[0], "-i");
    assert_eq!(args[2], "-o");
    assert_eq!(args[4], "-i");
    assert_eq!(args[6], "-o");
    assert!(args[1].ends_with("a.mp3"));
    assert!(args[3].ends_with("a.mp4"));
    assert!(args[5].ends_with("b.wav"));
    assert!(args[7].ends_with("b.mp4"));
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
