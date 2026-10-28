#![cfg(feature = "projectm")]

use std::path::{Path, PathBuf};

use pmvid::config::RenderConfig;
use pmvid::renderer::Renderer;

fn config() -> RenderConfig {
    RenderConfig {
        input: PathBuf::from("unused.mp3"),
        output: PathBuf::from("unused.mp4"),
        width: 64,
        height: 64,
        fps: 30,
        presets: PathBuf::from("tests/fixtures"),
        preset_duration: 20.0,
        beat_sensitivity: 1.0,
        seed: 0,
        crf: 20,
        ffmpeg: PathBuf::from("ffmpeg"),
    }
}

#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn records_a_broken_preset_and_keeps_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.milk");
    std::fs::write(&broken, b"this is not a milkdrop preset").unwrap();

    let mut renderer = Renderer::new(&config(), Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");

    renderer
        .load_preset(&broken, false)
        .expect("load_preset should not fail for a compile error");

    // Render a frame so any deferred failure is processed.
    let mut frame = vec![0_u8; renderer.frame_bytes()];
    renderer.render_frame(0.0, &mut frame).unwrap();

    let failures = renderer.failed_presets();
    assert!(
        failures.iter().any(|name| name.contains("broken.milk")),
        "expected broken.milk in failures, got {failures:?}"
    );
}

#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn missing_preset_file_is_an_error() {
    let renderer = Renderer::new(&config(), Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");

    let err = renderer
        .load_preset(Path::new("tests/fixtures/does-not-exist.milk"), false)
        .unwrap_err();
    assert!(format!("{err:#}").contains("does not exist"));
}
