//! ffmpeg process management.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};

use anyhow::{ensure, Context, Result};

/// Everything needed to build the ffmpeg command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderConfig {
    pub ffmpeg: PathBuf,
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub crf: u8,
}

/// Number of bytes in one RGBA frame for the given resolution.
pub(crate) fn rgba_frame_bytes(width: u32, height: u32) -> Result<usize> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .context("video resolution is too large")?;
    pixels.checked_mul(4).context("frame size overflow")
}
impl EncoderConfig {
    /// Number of bytes in one RGBA frame.
    pub fn frame_bytes(&self) -> Result<usize> {
        rgba_frame_bytes(self.width, self.height)
    }

    /// Arguments passed to ffmpeg. Does not include the program path.
    pub fn args(&self) -> Vec<OsString> {
        let mut args = Vec::new();

        args.push("-y".into());
        args.push("-f".into());
        args.push("rawvideo".into());
        args.push("-pix_fmt".into());
        args.push("rgba".into());
        args.push("-s".into());
        args.push(format!("{}x{}", self.width, self.height).into());
        args.push("-r".into());
        args.push(self.fps.to_string().into());
        args.push("-i".into());
        args.push("-".into());

        args.push("-i".into());
        args.push(self.input.as_os_str().to_owned());

        args.push("-map".into());
        args.push("0:v:0".into());
        args.push("-map".into());
        args.push("1:a:0".into());

        args.push("-c:v".into());
        args.push("libx264".into());
        args.push("-pix_fmt".into());
        args.push("yuv420p".into());
        args.push("-crf".into());
        args.push(self.crf.to_string().into());

        // OpenGL reads pixels bottom-up. Let ffmpeg flip them.
        args.push("-vf".into());
        args.push("vflip".into());

        args.push("-c:a".into());
        args.push("aac".into());
        args.push("-b:a".into());
        args.push("192k".into());
        args.push("-shortest".into());

        args.push(self.output.as_os_str().to_owned());

        args
    }
}

/// A running ffmpeg process that accepts raw RGBA frames on stdin.
#[derive(Debug)]
pub struct Encoder {
    child: Child,
    stdin: Option<ChildStdin>,
    frame_bytes: usize,
    output: PathBuf,
}

impl Encoder {
    /// Spawn ffmpeg with the configured arguments. Frames are written to its stdin.
    pub fn spawn(config: &EncoderConfig) -> Result<Self> {
        let frame_bytes = config.frame_bytes()?;

        let mut child = Command::new(&config.ffmpeg)
            .args(config.args())
            .stdin(Stdio::piped())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn ffmpeg at '{}'; install ffmpeg or pass --ffmpeg <PATH>",
                    config.ffmpeg.display()
                )
            })?;

        let stdin = child.stdin.take().context("ffmpeg stdin was not piped")?;

        Ok(Self {
            child,
            stdin: Some(stdin),
            frame_bytes,
            output: config.output.clone(),
        })
    }

    /// Write exactly one raw RGBA frame to ffmpeg. The frame length must equal `frame_bytes`.
    pub fn write_frame(&mut self, frame: &[u8]) -> Result<()> {
        ensure!(
            frame.len() == self.frame_bytes,
            "frame has {} bytes, expected {}",
            frame.len(),
            self.frame_bytes
        );

        let stdin = self
            .stdin
            .as_mut()
            .context("ffmpeg stdin is already closed")?;
        stdin
            .write_all(frame)
            .context("writing frame to ffmpeg (did ffmpeg exit early?)")?;
        Ok(())
    }

    /// Close stdin and wait for ffmpeg to finish writing the output file.
    pub fn finish(mut self) -> Result<()> {
        drop(self.stdin.take());

        let status = self
            .child
            .wait()
            .with_context(|| format!("waiting for ffmpeg to finish '{}'", self.output.display()))?;

        ensure!(
            status.success(),
            "ffmpeg exited with {status}; output '{}' may be incomplete",
            self.output.display()
        );

        Ok(())
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // If a render error unwinds before finish(), do not leave ffmpeg running.
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> EncoderConfig {
        EncoderConfig {
            ffmpeg: "ffmpeg".into(),
            input: "song.mp3".into(),
            output: "song.mp4".into(),
            width: 1280,
            height: 720,
            fps: 30,
            crf: 20,
        }
    }

    #[test]
    fn builds_the_expected_argument_list() {
        let args: Vec<String> = config()
            .args()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        let expected = vec![
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            "1280x720",
            "-r",
            "30",
            "-i",
            "-",
            "-i",
            "song.mp3",
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "20",
            "-vf",
            "vflip",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-shortest",
            "song.mp4",
        ];

        assert_eq!(args, expected);
    }

    #[test]
    fn frame_size_is_width_times_height_times_four() {
        assert_eq!(config().frame_bytes().unwrap(), 1280 * 720 * 4);
    }

    #[test]
    fn frame_size_uses_the_actual_resolution() {
        let cfg = EncoderConfig {
            width: 854,
            height: 480,
            ..config()
        };
        assert_eq!(cfg.frame_bytes().unwrap(), 854 * 480 * 4);
    }

    #[test]
    fn frame_size_overflow_is_an_error() {
        let cfg = EncoderConfig {
            width: u32::MAX,
            height: u32::MAX,
            ..config()
        };
        let err = cfg.frame_bytes().unwrap_err();
        assert!(format!("{err:#}").contains("frame size overflow"));
    }

    #[test]
    fn crf_and_fps_are_stringified() {
        let cfg = EncoderConfig {
            fps: 24,
            crf: 18,
            ..config()
        };
        let args: Vec<String> = cfg
            .args()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|pair| pair == ["-r", "24"]));
        assert!(args.windows(2).any(|pair| pair == ["-crf", "18"]));
    }
}
