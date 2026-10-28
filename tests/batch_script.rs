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
