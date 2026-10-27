use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::Result;
use pmvid::gl_ctx::GlContext;

fn main() -> Result<()> {
    let width = 64;
    let height = 64;
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "gl_smoke.ppm".to_string());

    let gl = GlContext::new(width, height)?;
    gl.bind_fbo();

    unsafe {
        gl::ClearColor(0.2, 0.4, 0.6, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    let mut rgba = vec![0_u8; gl.frame_bytes()];
    gl.read_pixels_rgba(&mut rgba)?;

    let first = &rgba[0..4];
    println!("first pixel: {first:?}");
    assert!(
        first[0] > 0 || first[1] > 0 || first[2] > 0,
        "readback returned a black pixel"
    );

    write_ppm(Path::new(&output), width, height, &rgba)?;
    println!("wrote {output}");
    Ok(())
}

fn write_ppm(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let mut file = BufWriter::new(File::create(path)?);
    write!(file, "P6\n{width} {height}\n255\n")?;

    // OpenGL rows are bottom-up; PPM is top-down.
    for y in (0..height).rev() {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            file.write_all(&rgba[index..index + 3])?;
        }
    }
    Ok(())
}
