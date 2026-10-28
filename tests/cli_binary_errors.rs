#![cfg(feature = "projectm")]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn no_arguments_fails_with_usage() {
    Command::cargo_bin("pmvid")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn missing_input_file_fails() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.mp4");

    Command::cargo_bin("pmvid")
        .unwrap()
        .args([
            "-i",
            dir.path().join("missing.mp3").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("input file"));
}

#[test]
fn corrupt_audio_fails_before_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("broken.mp3");
    fs::write(&input, b"not audio").unwrap();
    let output = dir.path().join("out.mp4");

    Command::cargo_bin("pmvid")
        .unwrap()
        .args([
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--presets",
            "tests/fixtures",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unsupported or corrupt audio file",
        ));
}
