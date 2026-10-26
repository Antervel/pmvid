# 0020 — Feed PCM and set frame time

- **Milestone:** M1
- **Depends on:** 0019, 0003, 0007
- **Files:** `src/audio.rs`, `src/renderer.rs`
- **Feature:** `projectm`

## Goal

For a given video frame, slice the matching audio window out of the prepared
PCM and feed it to projectM in chunks small enough for its internal buffer.

projectM's `pcm_add_float` stores a limited number of samples per channel
(`ProjectM::pcm_get_max_samples()`). A 30 fps frame is 1,470 samples per
channel, which is often larger than that buffer. Feed chunks, not the whole
frame at once.

## Rules

- KISS: one pure chunking function, one `Renderer` method.
- Every behaviour change needs a test; `cargo test` must pass.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` before finishing.
- Native checks are listed below.

## Implementation

### 1. Add a pure chunker to `src/audio.rs`

```rust
/// Split interleaved stereo samples into chunks that fit projectM's buffer.
///
/// `max_samples_per_channel` is `ProjectM::pcm_get_max_samples()`. The wrapper
/// asserts on the *total* sample count, so the chunk frame count is
/// `max_samples_per_channel / 2`, capped at 512 frames for responsiveness.
pub fn pcm_chunks(
    samples: &[f32],
    max_samples_per_channel: usize,
) -> impl Iterator<Item = &[f32]> {
    debug_assert!(
        max_samples_per_channel >= crate::CHANNELS,
        "projectM PCM buffer is too small for stereo"
    );
    let chunk_frames = (max_samples_per_channel / crate::CHANNELS).clamp(1, 512);
    samples.chunks(chunk_frames * crate::CHANNELS)
}
```

### 2. Add `feed_audio` to `src/renderer.rs`

```rust
use projectm::core::STEREO;

use crate::audio::pcm_chunks;
use crate::timebase::frame_sample_range;
use crate::CHANNELS;

impl Renderer {
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
        let max_samples = ProjectM::pcm_get_max_samples() as usize;
        for chunk in pcm_chunks(window, max_samples) {
            self.pm.pcm_add_float(chunk, STEREO);
        }

        Ok(())
    }
}
```

The existing `render_frame` already calls `projectm_set_frame_time` with
`frame_time_seconds(frame, fps)`. The app in ticket `0021` passes that value.

## Tests

### Chunker tests in `src/audio.rs`

```rust
#[test]
fn pcm_chunks_fit_the_projectm_buffer() {
    let samples = vec![0.0_f32; 3_000 * 2];
    let chunks: Vec<_> = pcm_chunks(&samples, 2_048).collect();

    assert_eq!(chunks.len(), 6); // 5 * 512 frames + 440 frames
    assert!(chunks.iter().all(|chunk| chunk.len() <= 2_048));
    assert_eq!(
        chunks.iter().map(|chunk| chunk.len()).sum::<usize>(),
        samples.len()
    );
}

#[test]
fn pcm_chunks_use_smaller_buffers_when_needed() {
    let samples = vec![0.0_f32; 1_000 * 2];
    let chunks: Vec<_> = pcm_chunks(&samples, 512).collect();

    assert!(chunks.iter().all(|chunk| chunk.len() <= 512));
    assert_eq!(
        chunks.iter().map(|chunk| chunk.len()).sum::<usize>(),
        samples.len()
    );
}

#[test]
fn pcm_chunks_cap_at_512_frames() {
    let samples = vec![0.0_f32; 2_000 * 2];
    let chunks: Vec<_> = pcm_chunks(&samples, 1_000_000).collect();
    assert_eq!(chunks[0].len(), 512 * 2);
}

#[test]
fn pcm_chunks_handle_empty_input() {
    let chunks: Vec<_> = pcm_chunks(&[], 2_048).collect();
    assert!(chunks.is_empty());
}
```

### Optional native test

If you want to exercise `feed_audio` without a real render, add an ignored test
to `tests/renderer_smoke.rs` that feeds one second of silence:

```rust
#[test]
#[ignore = "requires EGL/GL, projectM, and the native projectm feature"]
fn feeds_a_second_of_audio() {
    let config = config();
    let renderer = Renderer::new(&config, Path::new("tests/fixtures/simple.milk"))
        .expect("renderer initialization failed");
    let pcm = vec![0.0_f32; 44_100 * 2];

    for frame in 0..30 {
        renderer.feed_audio(frame, &pcm).unwrap();
    }
}
```

## Tests to run

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --all-targets audio

cargo clippy --all-targets --features projectm -- -D warnings
cargo test --all-targets --features projectm
LIBGL_ALWAYS_SOFTWARE=1 cargo test --features projectm -- --ignored --nocapture
```

## Acceptance criteria

- [ ] Every chunk passed to `pcm_add_float` fits projectM's buffer.
- [ ] Chunking never drops or duplicates samples.
- [ ] Chunks are at most 512 frames.
- [ ] `feed_audio` rejects a PCM buffer with an odd length.
- [ ] `feed_audio` rejects a frame whose window is past the end.
- [ ] Frame ranges come from `timebase::frame_sample_range`, not a rounded
      samples-per-frame constant.

## Notes for the implementer

- `ProjectM::pcm_add_float` in the `projectm` crate asserts on the **total**
  interleaved length, even though the C API's `count` is per channel. The
  chunker divides by 2 to satisfy both.
- Do not call `pcm_add_float` with a full 1,470-sample frame. It may panic or
  silently discard data, depending on the version.
- If the ignored native test panics with "Number of samples is greater than
  max samples", lower the chunk cap from 512 to 256 and re-run. Record the
  working value in `docs/spike-projectm.md`.
- Do not feed audio after the last frame. The render loop's total frame count
  guarantees `end <= pcm.len()`.
