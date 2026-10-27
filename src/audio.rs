//! Audio decoding and conversion.

use std::fs::File;
use std::path::Path;

use crate::SAMPLE_RATE;
use anyhow::{anyhow, bail, ensure, Context, Result};
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

/// Convert interleaved samples to interleaved stereo.
///
/// Mono is duplicated. Stereo is copied. For more than two channels, the first
/// two channels are kept.
pub fn to_stereo(samples: &[f32], channels: usize) -> Result<Vec<f32>> {
    ensure!(channels > 0, "channel count must be greater than zero");
    ensure!(
        samples.len() % channels == 0,
        "sample buffer length {} is not divisible by channel count {}",
        samples.len(),
        channels
    );

    match channels {
        1 => {
            let mut out = Vec::with_capacity(samples.len() * 2);
            for &sample in samples {
                out.push(sample);
                out.push(sample);
            }
            Ok(out)
        }
        2 => Ok(samples.to_vec()),
        _ => {
            let frames = samples.len() / channels;
            let mut out = Vec::with_capacity(frames * 2);
            for frame in samples.chunks_exact(channels) {
                out.push(frame[0]);
                out.push(frame[1]);
            }
            Ok(out)
        }
    }
}

/// Resample interleaved audio with linear interpolation.
///
/// `input.len()` must be divisible by `channels`. The output frame count is
/// `floor(input_frames * to_rate / from_rate)`.
pub fn resample_linear(
    input: &[f32],
    from_rate: u32,
    to_rate: u32,
    channels: usize,
) -> Result<Vec<f32>> {
    ensure!(
        from_rate > 0,
        "source sample rate must be greater than zero"
    );
    ensure!(to_rate > 0, "target sample rate must be greater than zero");
    ensure!(channels > 0, "channel count must be greater than zero");
    ensure!(
        input.len() % channels == 0,
        "sample buffer length {} is not divisible by channel count {}",
        input.len(),
        channels
    );

    if from_rate == to_rate || input.is_empty() {
        return Ok(input.to_vec());
    }

    let in_frames = input.len() / channels;
    let out_frames = (in_frames as u128) * u128::from(to_rate) / u128::from(from_rate);
    let ratio = f64::from(from_rate) / f64::from(to_rate);

    let out_frames = usize::try_from(out_frames)
        .map_err(|_| anyhow!("resampled frame count overflows usize"))?;
    let out_len = out_frames.checked_mul(channels).ok_or_else(|| {
        anyhow!(
            "resampled buffer size {} * {} overflows usize",
            out_frames,
            channels
        )
    })?;
    let mut out = vec![0.0_f32; out_len];

    for out_frame in 0..out_frames {
        let position = out_frame as f64 * ratio;
        let left = position.floor() as usize;
        let right = (left + 1).min(in_frames - 1);
        let fraction = (position - left as f64) as f32;

        for channel in 0..channels {
            let a = input[left * channels + channel];
            let b = input[right * channels + channel];
            out[out_frame * channels + channel] = a + (b - a) * fraction;
        }
    }

    Ok(out)
}

/// Audio ready for projectM: interleaved stereo `f32` at 44.1 kHz.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
}

impl PreparedAudio {
    /// Number of sample frames (samples per channel).
    pub fn frames(&self) -> u64 {
        debug_assert!(self.channels > 0);
        (self.samples.len() / self.channels) as u64
    }
}

/// Decode any supported file, convert it to stereo, and resample to 44.1 kHz.
pub fn prepare(path: &Path) -> Result<PreparedAudio> {
    let decoded = decode_file(path)?;
    let stereo = to_stereo(&decoded.samples, decoded.channels)?;
    let samples = resample_linear(&stereo, decoded.sample_rate, SAMPLE_RATE, 2)?;

    ensure!(
        !samples.is_empty(),
        "audio file '{}' produced no samples after resampling",
        path.display()
    );

    Ok(PreparedAudio {
        samples,
        sample_rate: SAMPLE_RATE,
        channels: 2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_duplicated() {
        let out = to_stereo(&[0.1, -0.2, 0.3], 1).unwrap();
        assert_eq!(out, vec![0.1, 0.1, -0.2, -0.2, 0.3, 0.3]);
    }

    #[test]
    fn stereo_is_copied() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let out = to_stereo(&input, 2).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn extra_channels_keep_front_left_and_right() {
        // 4-channel frames: FL, FR, RL, RR
        let input = vec![
            1.0, 2.0, 3.0, 4.0, //
            5.0, 6.0, 7.0, 8.0,
        ];
        let out = to_stereo(&input, 4).unwrap();
        assert_eq!(out, vec![1.0, 2.0, 5.0, 6.0]);
    }

    #[test]
    fn rejects_zero_channels() {
        assert!(to_stereo(&[0.0], 0).is_err());
    }

    #[test]
    fn rejects_partial_frame() {
        assert!(to_stereo(&[0.0, 0.0, 0.0], 2).is_err());
    }

    #[test]
    fn empty_input_is_empty_stereo() {
        assert_eq!(to_stereo(&[], 2).unwrap(), Vec::<f32>::new());
    }

    #[test]
    fn same_rate_is_identity() {
        let input = vec![0.1, -0.1, 0.2, -0.2];
        let out = resample_linear(&input, 44_100, 44_100, 2).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn forty_eight_k_to_forty_four_one_k_has_expected_length() {
        let input = vec![0.0_f32; 48_000 * 2];
        let out = resample_linear(&input, 48_000, 44_100, 2).unwrap();
        assert_eq!(out.len(), 44_100 * 2);
    }

    #[test]
    fn forty_four_one_k_to_forty_eight_k_has_expected_length() {
        let input = vec![0.0_f32; 44_100 * 2];
        let out = resample_linear(&input, 44_100, 48_000, 2).unwrap();
        assert_eq!(out.len(), 48_000 * 2);
    }

    #[test]
    fn constant_signal_is_preserved() {
        let input = vec![0.5_f32; 1_000 * 2];
        let out = resample_linear(&input, 48_000, 44_100, 2).unwrap();
        assert!(out.iter().all(|&sample| (sample - 0.5).abs() < 1e-6));
    }

    #[test]
    fn channels_are_resampled_independently() {
        // 2 frames: L = 0.25, R = -0.25
        let input = vec![0.25, -0.25, 0.25, -0.25];
        let out = resample_linear(&input, 44_100, 22_050, 2).unwrap();
        assert_eq!(out, vec![0.25, -0.25]); // one output frame, no interpolation needed
    }

    #[test]
    fn empty_input_stays_empty() {
        let out = resample_linear(&[], 48_000, 44_100, 2).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn rejects_zero_rates_and_channels() {
        assert!(resample_linear(&[0.0], 0, 44_100, 1).is_err());
        assert!(resample_linear(&[0.0], 44_100, 0, 1).is_err());
        assert!(resample_linear(&[0.0], 44_100, 44_100, 0).is_err());
    }

    #[test]
    fn resample_rejects_partial_frame() {
        assert!(resample_linear(&[0.0, 0.0, 0.0], 44_100, 44_100, 2).is_err());
    }
}
