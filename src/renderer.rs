//! projectM renderer: GL context + projectM instance + FBO readback.

use std::path::Path;

use anyhow::{ensure, Context, Result};
use projectm::core::ProjectM;

use crate::config::RenderConfig;
use crate::gl_ctx::GlContext;

/// Owns the GL context and projectM instance.
///
/// The GL context is created first and stays current for the lifetime of this
/// struct. Keep all rendering on the thread that constructed it.
pub struct Renderer {
    gl: GlContext,
    pm: ProjectM,
    fps: u32,
}

impl Renderer {
    /// Create the GL context, then projectM, then load the first preset.
    pub fn new(config: &RenderConfig, preset: &Path) -> Result<Self> {
        let gl = GlContext::new(config.width, config.height)
            .context("creating the headless OpenGL context")?;

        let pm = ProjectM::create();
        let handle = *pm.get_instance().borrow();
        ensure!(
            !handle.is_null(),
            "projectM failed to create an instance; the GL context may not be current"
        );

        pm.set_window_size(config.width as usize, config.height as usize);
        pm.set_fps(config.fps);
        pm.set_beat_sensitivity(config.beat_sensitivity);

        // We drive preset switching from the frame schedule. Stop projectM's
        // own timer from requesting switches.
        pm.set_preset_locked(true);

        let renderer = Self {
            gl,
            pm,
            fps: config.fps,
        };
        renderer.load_preset(preset, false)?;
        Ok(renderer)
    }

    /// Load a preset. `smooth` requests projectM's soft cut.
    pub fn load_preset(&self, preset: &Path, smooth: bool) -> Result<()> {
        let preset = preset
            .to_str()
            .with_context(|| format!("preset path is not valid UTF-8: {}", preset.display()))?;
        self.pm.load_preset_file(preset, smooth);
        Ok(())
    }

    /// Render one frame at `frame_time_s` and read it into `out`.
    ///
    /// `out` must be exactly `frame_bytes()` long. The rows are bottom-up; the
    /// ffmpeg command adds `-vf vflip`.
    pub fn render_frame(&mut self, frame_time_s: f64, out: &mut [u8]) -> Result<()> {
        ensure!(
            frame_time_s.is_finite() && frame_time_s >= 0.0,
            "frame time must be a finite, non-negative number"
        );

        let handle = *self.pm.get_instance().borrow();

        unsafe {
            // Deterministic offline timing. This is why ticket 0018 exists.
            projectm_sys::projectm_set_frame_time(handle, frame_time_s);
            // Render directly into our FBO, not the default framebuffer.
            projectm_sys::projectm_opengl_render_frame_fbo(handle, self.gl.fbo());
        }

        self.gl.read_pixels_rgba(out)?;
        Ok(())
    }

    /// Number of bytes in one RGBA frame.
    pub fn frame_bytes(&self) -> usize {
        self.gl.frame_bytes()
    }

    /// The fps projectM was configured with.
    pub fn fps(&self) -> u32 {
        self.fps
    }
}
