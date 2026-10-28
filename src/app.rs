//! Application orchestration.

use std::io::IsTerminal;

use anyhow::{ensure, Context, Result};
use clap::Parser;

use crate::audio;
use crate::cli::Cli;
use crate::config::RenderConfig;
use crate::encoder::Encoder;
use crate::presets::{self, PresetSchedule};
use crate::progress::Progress;
use crate::renderer::Renderer;
use crate::timebase::{frame_time_seconds, total_frames};

/// Parse the CLI and run a render.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = RenderConfig::from_cli(cli)?;
    run_with_config(config)
}

/// Run a render with an already-validated configuration.
///
/// This is public so the ignored end-to-end test can call it without
/// synthesizing command-line arguments.
pub fn run_with_config(config: RenderConfig) -> Result<()> {
    eprintln!("seed: {}", config.seed);

    let audio = audio::prepare(&config.input).context("preparing audio")?;
    let frame_count = total_frames(audio.frames(), config.fps);
    ensure!(
        frame_count > 0,
        "audio is too short to render at {} fps",
        config.fps
    );

    let preset_files = presets::scan(&config.presets).context("loading presets")?;
    let preset_files = presets::shuffle(preset_files, config.seed);
    let mut schedule = PresetSchedule::new(preset_files, config.preset_duration, config.fps)?;

    let mut renderer =
        Renderer::new(&config, schedule.current()).context("initializing the renderer")?;
    let mut encoder = Encoder::spawn(&config.encoder_config()).context("starting ffmpeg")?;

    let mut frame = vec![0_u8; config.frame_bytes()?];

    let progress = if std::io::stderr().is_terminal() {
        Progress::new(frame_count, config.fps)
    } else {
        Progress::hidden()
    };

    for frame_index in 0..frame_count {
        if let Some(preset) = schedule.advance(frame_index) {
            renderer.load_preset(preset, true)?;
        }

        renderer.feed_audio(frame_index, &audio.samples)?;

        let time = frame_time_seconds(frame_index, config.fps);
        renderer.render_frame(time, &mut frame)?;
        encoder.write_frame(&frame)?;

        progress.set_frame(frame_index + 1);
    }

    encoder.finish()?;
    progress.finish();

    eprintln!("wrote {}", config.output.display());
    Ok(())
}
