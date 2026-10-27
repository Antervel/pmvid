# 0006 — Resample to 44.1 kHz

- **Milestone:** M1
- **Depends on:** 0005
- **Files:** `src/audio.rs`
- **Feature:** default

## Goal

Resample interleaved stereo audio to 44,100 Hz with simple linear
interpolation. The visualizer only needs a stable analysis signal; this does
not need to be audiophile-grade. The original audio is muxed into the MP4 by
ffmpeg, so this resampling never affects what the listener hears.

## Rules

- KISS: linear interpolation, no windowed-sinc filter.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Add to `src/audio.rs`:

```rust
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
    ensure!(from_rate > 0, "source sample rate must be greater than zero");
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
    let out_frames =
        ((in_frames as u128 * u128::from(to_rate)) / u128::from(from_rate)) as usize;
    let ratio = f64::from(from_rate) / f64::from(to_rate);

    let mut out = vec![0.0_f32; out_frames * channels];

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
```

Add tests to the existing audio test module:

```rust
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
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets audio
cargo test --all-targets
```

## Acceptance criteria

- [x] 48 kHz → 44.1 kHz produces `44_100` frames for one second of input.
- [x] 44.1 kHz → 48 kHz produces `48_000` frames for one second of input.
- [x] Identical rates return the input unchanged.
- [x] Constant signals stay constant within `1e-6`.
- [x] Left and right channels do not leak into each other.
- [x] Zero rates/channels and partial frames are rejected.

## Notes for the implementer

- Use `u128` for the output-frame calculation. A 10-minute 192 kHz file times a
  target rate still fits in `u64`, but `u128` removes the question entirely.
- The last output frame may read `right == left`; that is intentional.
- Do not add `rubato` or another resampling crate. Linear interpolation is
  enough for visual analysis and keeps the dependency tree small.
- If the test `channels_are_resampled_independently` fails due to a different
  output length, check the `out_frames` formula before changing the test.
