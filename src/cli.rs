//! Command-line interface.

use std::path::PathBuf;

use anyhow::{ensure, Result};
use clap::Parser;

/// Render a projectM visualization for an audio file into an MP4.
#[derive(Debug, Clone, Parser)]
#[command(name = "pmvid", version, about, long_about = None)]
pub struct Cli {
    /// Input audio file.
    #[arg(short = 'i', long, value_name = "FILE")]
    pub input: PathBuf,

    /// Output video file.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: PathBuf,

    /// Output width in pixels.
    #[arg(short = 'W', long, value_name = "PX", default_value_t = 1280)]
    pub width: u32,

    /// Output height in pixels.
    #[arg(short = 'H', long, value_name = "PX", default_value_t = 720)]
    pub height: u32,

    /// Output frames per second.
    #[arg(short = 'f', long, value_name = "N", default_value_t = 30)]
    pub fps: u32,

    /// Directory containing .milk presets.
    #[arg(short = 'p', long, value_name = "DIR", default_value = "./presets")]
    pub presets: PathBuf,

    /// Seconds to show each preset.
    #[arg(long, value_name = "SEC", default_value_t = 20.0)]
    pub preset_duration: f64,

    /// projectM beat sensitivity.
    #[arg(long, value_name = "F", default_value_t = 1.0)]
    pub beat_sensitivity: f32,

    /// RNG seed for the preset order. Omit for a random seed.
    #[arg(long, value_name = "N")]
    pub seed: Option<u64>,

    /// x264 CRF quality (lower is better, 0-51).
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub crf: u8,

    /// ffmpeg binary to use.
    #[arg(long, value_name = "PATH", default_value = "ffmpeg")]
    pub ffmpeg: PathBuf,
}

impl Cli {
    /// Validate values that clap cannot express cleanly.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.width > 0 && self.height > 0,
            "width and height must be greater than zero"
        );
        ensure!(
            self.width % 2 == 0 && self.height % 2 == 0,
            "width and height must be even for H.264 yuv420p (got {}x{})",
            self.width,
            self.height
        );
        ensure!(self.fps > 0, "fps must be greater than zero");
        ensure!(
            self.preset_duration.is_finite() && self.preset_duration > 0.0,
            "preset duration must be a positive number of seconds"
        );
        ensure!(
            self.beat_sensitivity.is_finite() && self.beat_sensitivity > 0.0,
            "beat sensitivity must be a positive number"
        );
        ensure!(self.crf <= 51, "crf must be between 0 and 51");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn clap_configuration_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_required_arguments_and_defaults() {
        let cli = parse(&["pmvid", "-i", "song.mp3", "-o", "song.mp4"]);

        assert_eq!(cli.input, PathBuf::from("song.mp3"));
        assert_eq!(cli.output, PathBuf::from("song.mp4"));
        assert_eq!(cli.width, 1280);
        assert_eq!(cli.height, 720);
        assert_eq!(cli.fps, 30);
        assert_eq!(cli.presets, PathBuf::from("./presets"));
        assert_eq!(cli.preset_duration, 20.0);
        assert_eq!(cli.beat_sensitivity, 1.0);
        assert_eq!(cli.seed, None);
        assert_eq!(cli.crf, 20);
        assert_eq!(cli.ffmpeg, PathBuf::from("ffmpeg"));
    }

    #[test]
    fn parses_short_flags() {
        let cli = parse(&[
            "pmvid",
            "-i",
            "in.mp3",
            "-o",
            "out.mp4",
            "-W",
            "640",
            "-H",
            "360",
            "-f",
            "24",
            "-p",
            "my-presets",
        ]);

        assert_eq!(cli.width, 640);
        assert_eq!(cli.height, 360);
        assert_eq!(cli.fps, 24);
        assert_eq!(cli.presets, PathBuf::from("my-presets"));
    }

    #[test]
    fn parses_long_options() {
        let cli = parse(&[
            "pmvid",
            "--input",
            "in.mp3",
            "--output",
            "out.mp4",
            "--width",
            "1920",
            "--height",
            "1080",
            "--fps",
            "60",
            "--presets",
            "/tmp/presets",
            "--preset-duration",
            "12.5",
            "--beat-sensitivity",
            "1.5",
            "--seed",
            "42",
            "--crf",
            "18",
            "--ffmpeg",
            "/usr/bin/ffmpeg",
        ]);

        assert_eq!(cli.width, 1920);
        assert_eq!(cli.height, 1080);
        assert_eq!(cli.fps, 60);
        assert_eq!(cli.presets, PathBuf::from("/tmp/presets"));
        assert_eq!(cli.preset_duration, 12.5);
        assert_eq!(cli.beat_sensitivity, 1.5);
        assert_eq!(cli.seed, Some(42));
        assert_eq!(cli.crf, 18);
        assert_eq!(cli.ffmpeg, PathBuf::from("/usr/bin/ffmpeg"));
    }

    #[test]
    fn input_and_output_are_required() {
        assert!(Cli::try_parse_from(["pmvid"]).is_err());
        assert!(Cli::try_parse_from(["pmvid", "-i", "in.mp3"]).is_err());
        assert!(Cli::try_parse_from(["pmvid", "-o", "out.mp4"]).is_err());
    }

    #[test]
    fn rejects_non_numeric_values() {
        assert!(
            Cli::try_parse_from(["pmvid", "-i", "in.mp3", "-o", "out.mp4", "--width", "wide"])
                .is_err()
        );
        assert!(Cli::try_parse_from([
            "pmvid",
            "-i",
            "in.mp3",
            "-o",
            "out.mp4",
            "--seed",
            "not-a-number"
        ])
        .is_err());
    }
    fn valid_args() -> Vec<&'static str> {
        vec!["pmvid", "-i", "song.mp3", "-o", "song.mp4"]
    }

    #[test]
    fn defaults_are_valid() {
        let cli = parse(&valid_args());
        cli.validate().unwrap();
    }

    #[test]
    fn accepts_a_non_16_by_9_even_resolution() {
        let cli = parse(&[
            "pmvid", "-i", "in.mp3", "-o", "out.mp4", "-W", "854", "-H", "480",
        ]);
        cli.validate().unwrap();
    }

    #[test]
    fn rejects_zero_width_or_height() {
        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-W", "0"]);
        assert!(cli.validate().is_err());

        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-H", "0"]);
        assert!(cli.validate().is_err());
    }

    #[test]
    fn rejects_odd_dimensions() {
        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-W", "1279"]);
        let err = cli.validate().unwrap_err();
        assert!(format!("{err:#}").contains("must be even"));

        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-H", "721"]);
        assert!(cli.validate().is_err());
    }

    #[test]
    fn rejects_zero_fps() {
        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "-f", "0"]);
        let err = cli.validate().unwrap_err();
        assert!(format!("{err:#}").contains("fps must be greater than zero"));
    }

    #[test]
    fn rejects_bad_preset_duration() {
        // Use the `--flag=value` form: clap treats a separate `-1` token as an
        // unknown flag and would reject it during parsing, before `validate`.
        for value in ["0", "-1", "NaN", "inf"] {
            let arg = format!("--preset-duration={value}");
            let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", &arg]);
            assert!(cli.validate().is_err(), "accepted duration {value}");
        }
    }

    #[test]
    fn rejects_bad_beat_sensitivity() {
        for value in ["0", "-0.5", "NaN", "inf"] {
            let arg = format!("--beat-sensitivity={value}");
            let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", &arg]);
            assert!(cli.validate().is_err(), "accepted sensitivity {value}");
        }
    }

    #[test]
    fn rejects_crf_above_51() {
        let cli = parse(&["pmvid", "-i", "in.mp3", "-o", "out.mp4", "--crf", "52"]);
        let err = cli.validate().unwrap_err();
        assert!(format!("{err:#}").contains("crf must be between 0 and 51"));
    }

    #[test]
    fn rejects_empty_ffmpeg_path() {
        // clap's `PathBuf` value parser already rejects an empty value, so this
        // fails during parsing rather than in `validate`.
        assert!(
            Cli::try_parse_from(["pmvid", "-i", "in.mp3", "-o", "out.mp4", "--ffmpeg", ""])
                .is_err()
        );
    }
}
