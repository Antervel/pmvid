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
fn renders_a_non_black_frame() {
    let config = config();
    let mut renderer = Renderer::new(&config, Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");

    let mut frame = vec![0_u8; renderer.frame_bytes()];
    renderer
        .render_frame(0.0, &mut frame)
        .expect("frame render failed");

    let non_black = frame
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0)
        .count();

    assert!(
        non_black > 0,
        "renderer returned an all-black frame; check the FBO and preset"
    );
}

#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn rejects_a_negative_frame_time() {
    let config = config();
    let mut renderer = Renderer::new(&config, Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");
    let mut frame = vec![0_u8; renderer.frame_bytes()];

    assert!(renderer.render_frame(-0.1, &mut frame).is_err());
}

#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn feeds_a_second_of_audio() {
    let config = config();
    let renderer = Renderer::new(&config, Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");
    let pcm = vec![0.0_f32; 44_100 * 2];

    for frame in 0..30 {
        renderer.feed_audio(frame, &pcm).unwrap();
    }
}
