//! projectM renderer: GL context + projectM instance + FBO readback.

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{ensure, Context, Result};
use projectm::core::{ProjectM, STEREO};

use crate::audio::pcm_chunks;
use crate::config::RenderConfig;
use crate::gl_ctx::GlContext;
use crate::timebase::frame_sample_range;
use crate::CHANNELS;

/// Owns the GL context and projectM instance.
///
/// The GL context is created first and stays current for the lifetime of this
/// struct. Keep all rendering on the thread that constructed it.
pub struct Renderer {
    gl: GlContext,
    pm: ProjectM,
    fps: u32,
    failed_presets: Arc<Mutex<Vec<String>>>,
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

        let failed_presets = Arc::new(Mutex::new(Vec::new()));
        let failed_for_callback = Arc::clone(&failed_presets);

        pm.set_preset_switch_failed_event_callback(move |filename, message| {
            eprintln!("warning: preset '{filename}' failed to load: {message}");
            if let Ok(mut failures) = failed_for_callback.lock() {
                failures.push(filename);
            }
        });

        let renderer = Self {
            gl,
            pm,
            fps: config.fps,
            failed_presets,
        };
        renderer.load_preset(preset, false)?;
        Ok(renderer)
    }

    /// Load a preset. `smooth` requests projectM's soft cut.
    pub fn load_preset(&self, preset: &Path, smooth: bool) -> Result<()> {
        ensure!(
            preset.is_file(),
            "preset file '{}' does not exist",
            preset.display()
        );
        let preset_str = preset
            .to_str()
            .with_context(|| format!("preset path is not valid UTF-8: {}", preset.display()))?;
        let preset_c =
            std::ffi::CString::new(preset_str).context("preset path contains a NUL byte")?;
        let preset_str =
            std::str::from_utf8(preset_c.as_bytes()).context("preset path is not valid UTF-8")?;
        self.pm.load_preset_file(preset_str, smooth);
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

    /// Presets that projectM reported as failed since startup.
    pub fn failed_presets(&self) -> Vec<String> {
        self.failed_presets
            .lock()
            .map(|failures| failures.clone())
            .unwrap_or_default()
    }

    /// Feed the audio belonging to `frame` to projectM.
    ///
    /// `pcm` must be interleaved stereo at 44.1 kHz, as produced by
    /// `audio::prepare`.
    pub fn feed_audio(&self, frame: u64, pcm: &[f32]) -> Result<()> {
        ensure!(
            pcm.len() % CHANNELS == 0,
            "PCM buffer length {} is not divisible by {CHANNELS}",
            pcm.len()
        );

        let range = frame_sample_range(frame, self.fps);
        let start = range.start as usize * CHANNELS;
        let end = range.end as usize * CHANNELS;

        ensure!(
            end <= pcm.len(),
            "frame {frame} needs PCM samples {start}..{end}, but the buffer has {} samples",
            pcm.len()
        );

        let window = &pcm[start..end];
        let max_frames = ProjectM::pcm_get_max_samples() as usize / CHANNELS;
        for chunk in pcm_chunks(window, max_frames) {
            self.pm.pcm_add_float(chunk, STEREO);
        }

        Ok(())
    }
}
