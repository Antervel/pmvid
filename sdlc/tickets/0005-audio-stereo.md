# 0005 — Convert decoded audio to stereo

- **Milestone:** M1
- **Depends on:** 0004
- **Files:** `src/audio.rs`
- **Feature:** default

## Goal

Turn any decoded channel layout into interleaved stereo `f32`:

- mono → duplicate the channel to L/R
- stereo → leave it alone
- more than two channels → use the first two channels (front L/R)

This is intentionally simple. projectM only needs a stereo analysis signal; it
does not need a surround downmix.

## Rules

- KISS: no channel-layout metadata, no surround weighting.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.

## Implementation

Add this to `src/audio.rs`:

```rust
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
```

Add tests in the existing `#[cfg(test)] mod tests` at the bottom of
`src/audio.rs` (create the module if ticket `0004` did not add one):

```rust
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

- [x] Mono input produces `2 * input.len()` stereo samples.
- [x] Stereo input is returned unchanged.
- [x] Multi-channel input keeps the first two channels.
- [x] Zero channels and partial frames are rejected with clear errors.
- [x] Empty input is allowed and returns an empty vector.

## Notes for the implementer

- Do not add a `StereoAudio` newtype yet. A `Vec<f32>` plus the invariant
  "interleaved stereo" is enough for the render loop.
- The output length is always even. Ticket `0006` relies on that.
