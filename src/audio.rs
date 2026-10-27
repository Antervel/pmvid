//! Audio decoding and conversion.

use std::fs::File;
use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decoded audio in the file's native sample rate and channel layout.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudio {
    /// Interleaved samples. Length is `frames * channels`.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
}

impl DecodedAudio {
    /// Number of sample frames (samples per channel).
    pub fn frames(&self) -> u64 {
        if self.channels == 0 {
            0
        } else {
            (self.samples.len() / self.channels) as u64
        }
    }
}

/// Decode an audio file to interleaved `f32` at its native sample rate.
///
/// The samples are interleaved across channels; no resampling or downmixing is performed, so the
/// output keeps the file's own sample rate and channel count.
pub fn decode_file(path: &Path) -> Result<DecodedAudio> {
    let file =
        File::open(path).with_context(|| format!("opening audio file '{}'", path.display()))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| {
            format!(
                "unsupported or corrupt audio file '{}' (expected MP3, WAV, FLAC, or OGG)",
                path.display()
            )
        })?;

    let mut format = probed.format;

    let track = format
        .default_track()
        .with_context(|| format!("no audio track found in '{}'", path.display()))?;
    let (track_id, codec_params) = (track.id, track.codec_params.clone());

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .with_context(|| format!("unsupported audio codec in '{}'", path.display()))?;

    let mut samples = Vec::new();
    let mut sample_rate: Option<u32> = None;
    let mut channels: Option<usize> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                bail!("audio stream changed format mid-file; this is not supported");
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("reading audio from '{}'", path.display()));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();

                if let Some(rate) = sample_rate {
                    ensure!(
                        rate == spec.rate,
                        "sample rate changed from {rate} Hz to {} Hz mid-file",
                        spec.rate
                    );
                }
                if let Some(count) = channels {
                    ensure!(
                        count == spec.channels.count(),
                        "channel count changed mid-file"
                    );
                }

                sample_rate = Some(spec.rate);
                channels = Some(spec.channels.count());

                // Allocate per packet. This is simple and correct; optimize only
                // if profiling shows it matters.
                let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buffer.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buffer.samples());
            }
            Err(SymphoniaError::DecodeError(message)) => {
                eprintln!("warning: skipping corrupt audio frame: {message}");
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("decoding audio from '{}'", path.display()));
            }
        }
    }

    let sample_rate = sample_rate.context("audio stream has no sample rate")?;
    let channels = channels.context("audio stream has no channel layout")?;

    ensure!(
        channels > 0,
        "audio stream in '{}' has zero channels",
        path.display()
    );
    ensure!(
        !samples.is_empty(),
        "decoded zero audio samples from '{}'",
        path.display()
    );

    Ok(DecodedAudio {
        samples,
        sample_rate,
        channels,
    })
}
