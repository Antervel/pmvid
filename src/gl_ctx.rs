//! Headless EGL context and an offscreen RGBA framebuffer.

use std::ffi::c_void;

use anyhow::{bail, ensure, Context, Result};
use khronos_egl as egl;

/// Mesa's surfaceless platform enum. Not exposed by khronos-egl.
const EGL_PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;

/// A current headless GL context plus a complete FBO.
pub struct GlContext {
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
    surface: Option<egl::Surface>,
    fbo: gl::types::GLuint,
    color_texture: gl::types::GLuint,
    depth_stencil: gl::types::GLuint,
    width: u32,
    height: u32,
}

impl GlContext {
    /// Create a surfaceless EGL context and an FBO of the given size.
    ///
    /// The context is made current on the calling thread. Keep all GL work on
    /// this thread for the lifetime of the object.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        ensure!(width > 0 && height > 0, "GL size must be non-zero");

        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
            .context("loading libEGL.so.1; install Mesa EGL (libegl1-mesa-dev)")?;

        // Prefer the surfaceless platform. If the driver does not expose it,
        // fall back to the default display (useful under xvfb-run).
        let display = match unsafe {
            egl.get_platform_display(
                EGL_PLATFORM_SURFACELESS_MESA,
                egl::DEFAULT_DISPLAY,
                &[egl::ATTRIB_NONE],
            )
        } {
            Ok(display) => display,
            Err(_) => unsafe { egl.get_display(egl::DEFAULT_DISPLAY) }
                .context("creating an EGL display; install Mesa EGL or run under xvfb-run")?,
        };

        let (major, minor) = egl.initialize(display).context("initializing EGL")?;
        eprintln!("EGL {major}.{minor}");

        egl.bind_api(egl::OPENGL_API)
            .context("binding the OpenGL API to the EGL context")?;

        let config_attributes = [
            egl::SURFACE_TYPE,
            egl::PBUFFER_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::DEPTH_SIZE,
            24,
            egl::STENCIL_SIZE,
            8,
            egl::NONE,
        ];
        let config = egl
            .choose_first_config(display, &config_attributes)
            .context("choosing an EGL config")?
            .context("no EGL config supports OpenGL + pbuffer")?;

        let context_attributes = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            3,
            egl::CONTEXT_OPENGL_PROFILE_MASK,
            egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
            egl::NONE,
        ];
        let context = egl
            .create_context(display, config, None, &context_attributes)
            .context("creating an OpenGL 3.3 core context")?;

        let surface_attributes = [
            egl::WIDTH,
            width as egl::Int,
            egl::HEIGHT,
            height as egl::Int,
            egl::NONE,
        ];
        let surface = egl
            .create_pbuffer_surface(display, config, &surface_attributes)
            .context("creating a pbuffer surface")?;

        egl.make_current(display, Some(surface), Some(surface), Some(context))
            .context("making the EGL context current")?;

        gl::load_with(|name| {
            egl.get_proc_address(name)
                .map(|function| function as *const c_void)
                .unwrap_or(std::ptr::null())
        });

        let version = unsafe {
            std::ffi::CStr::from_ptr(gl::GetString(gl::VERSION) as *const std::os::raw::c_char)
                .to_string_lossy()
                .into_owned()
        };
        eprintln!("GL {version}");

        let mut gl_ctx = Self {
            egl,
            display,
            context,
            surface: Some(surface),
            fbo: 0,
            color_texture: 0,
            depth_stencil: 0,
            width,
            height,
        };
        gl_ctx.create_fbo()?;
        Ok(gl_ctx)
    }

    fn create_fbo(&mut self) -> Result<()> {
        unsafe {
            gl::GenFramebuffers(1, &mut self.fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);

            gl::GenTextures(1, &mut self.color_texture);
            gl::BindTexture(gl::TEXTURE_2D, self.color_texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as i32,
                self.width as i32,
                self.height as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                self.color_texture,
                0,
            );

            gl::GenRenderbuffers(1, &mut self.depth_stencil);
            gl::BindRenderbuffer(gl::RENDERBUFFER, self.depth_stencil);
            gl::RenderbufferStorage(
                gl::RENDERBUFFER,
                gl::DEPTH24_STENCIL8,
                self.width as i32,
                self.height as i32,
            );
            gl::FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_STENCIL_ATTACHMENT,
                gl::RENDERBUFFER,
                self.depth_stencil,
            );

            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            if status != gl::FRAMEBUFFER_COMPLETE {
                bail!("framebuffer is incomplete: 0x{status:x}");
            }

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
        Ok(())
    }

    /// Bind the offscreen framebuffer and set the viewport to its size.
    pub fn bind_fbo(&self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::Viewport(0, 0, self.width as i32, self.height as i32);
        }
    }

    /// Raw FBO id, for `projectm_opengl_render_frame_fbo`.
    pub fn fbo(&self) -> u32 {
        self.fbo
    }

    /// Number of bytes in one RGBA frame.
    pub fn frame_bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// Read the current FBO into `out` as tightly packed RGBA bytes.
    ///
    /// The rows are bottom-up, as OpenGL returns them.
    pub fn read_pixels_rgba(&self, out: &mut [u8]) -> Result<()> {
        ensure!(
            out.len() == self.frame_bytes(),
            "pixel buffer has {} bytes, expected {}",
            out.len(),
            self.frame_bytes()
        );

        self.bind_fbo();
        unsafe {
            gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
            gl::ReadPixels(
                0,
                0,
                self.width as i32,
                self.height as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                out.as_mut_ptr() as *mut c_void,
            );
        }
        Ok(())
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::DeleteFramebuffers(1, &self.fbo);
            gl::DeleteTextures(1, &self.color_texture);
            gl::DeleteRenderbuffers(1, &self.depth_stencil);
        }

        let _ = self.egl.make_current(self.display, None, None, None);
        if let Some(surface) = self.surface.take() {
            let _ = self.egl.destroy_surface(self.display, surface);
        }
        let _ = self.egl.destroy_context(self.display, self.context);
        let _ = self.egl.terminate(self.display);
    }
}
