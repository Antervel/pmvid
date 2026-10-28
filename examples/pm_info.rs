use anyhow::{ensure, Result};
use pmvid::gl_ctx::GlContext;
use projectm::core::ProjectM;

fn main() -> Result<()> {
    let width = 64_u32;
    let height = 64_u32;

    // The GL context must be current before projectM is created.
    let gl = GlContext::new(width, height)?;

    let pm = ProjectM::create();
    let handle = *pm.get_instance().borrow();
    ensure!(
        !handle.is_null(),
        "projectM_create returned NULL; is the GL context current?"
    );

    println!("projectM version: {}", ProjectM::get_version_string());
    println!(
        "pcm max samples (per channel): {}",
        ProjectM::pcm_get_max_samples()
    );

    pm.set_window_size(width as usize, height as usize);
    pm.set_fps(30);
    pm.set_beat_sensitivity(1.0);

    let preset = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tests/fixtures/simple.milk".to_string());
    println!("loading preset: {preset}");
    pm.load_preset_file(&preset, false);

    // Available in projectM 4.2+. If this does not compile, read the ticket
    // notes and stop.
    unsafe {
        projectm_sys::projectm_set_frame_time(handle, 0.0);
    }

    gl.bind_fbo();
    // Available in projectM 4.2+. If this does not compile, use
    // `pm.render_frame()` with the FBO bound and record the result.
    unsafe {
        projectm_sys::projectm_opengl_render_frame_fbo(handle, gl.fbo());
    }

    let mut rgba = vec![0_u8; gl.frame_bytes()];
    gl.read_pixels_rgba(&mut rgba)?;

    let non_black = rgba
        .chunks_exact(4)
        .filter(|pixel| pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0)
        .count();
    println!("non-black pixels: {}/{}", non_black, width * height);

    ensure!(
        non_black > 0,
        "projectM rendered an all-black frame; check the FBO binding and preset"
    );

    println!("projectM spike OK");
    Ok(())
}
