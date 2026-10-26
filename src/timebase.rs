//! Integer timebase math shared by the audio and render loops.

use std::ops::Range;

use crate::SAMPLE_RATE;

/// Number of video frames needed to cover `total_samples_per_channel` audio.
///
/// The result is rounded down, so the last frame never reads past the end of
/// the decoded PCM buffer.
pub fn total_frames(total_samples_per_channel: u64, fps: u32) -> u64 {
    debug_assert!(fps > 0, "fps must be positive");
    total_samples_per_channel * u64::from(fps) / u64::from(SAMPLE_RATE)
}

/// Half-open range of audio samples (per channel) for a video frame.
///
/// The ranges for consecutive frames are contiguous:
/// `frame_sample_range(n).end == frame_sample_range(n + 1).start`.
pub fn frame_sample_range(frame: u64, fps: u32) -> Range<u64> {
    debug_assert!(fps > 0, "fps must be positive");
    let rate = u64::from(SAMPLE_RATE);
    let fps = u64::from(fps);
    let start = frame * rate / fps;
    let end = (frame + 1) * rate / fps;
    start..end
}

/// Presentation time of a frame, in seconds, for projectM's fixed frame time.
pub fn frame_time_seconds(frame: u64, fps: u32) -> f64 {
    debug_assert!(fps > 0, "fps must be positive");
    frame as f64 / f64::from(fps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_fps_has_1470_samples_per_frame() {
        assert_eq!(frame_sample_range(0, 30), 0..1_470);
        assert_eq!(frame_sample_range(1, 30), 1_470..2_940);
        assert_eq!(frame_sample_range(29, 30), 42_630..44_100);
    }

    #[test]
    fn twenty_four_fps_ranges_are_contiguous() {
        let mut previous_end = 0;
        for frame in 0..100 {
            let range = frame_sample_range(frame, 24);
            assert_eq!(range.start, previous_end, "gap before frame {frame}");
            assert!(range.end > range.start, "empty range at frame {frame}");
            previous_end = range.end;
        }
    }

    #[test]
    fn sixty_fps_ranges_are_contiguous() {
        let mut previous_end = 0;
        for frame in 0..100 {
            let range = frame_sample_range(frame, 60);
            assert_eq!(range.start, previous_end, "gap before frame {frame}");
            assert!(range.end > range.start, "empty range at frame {frame}");
            previous_end = range.end;
        }
    }

    #[test]
    fn ten_minutes_at_thirty_fps_has_no_drift() {
        // 10 minutes * 60 s * 44,100 Hz = 26,460,000 samples.
        let total_samples = 26_460_000;
        let frames = total_frames(total_samples, 30);
        assert_eq!(frames, 18_000);
        assert_eq!(frame_sample_range(frames - 1, 30).end, total_samples);
    }

    #[test]
    fn ten_minutes_at_twenty_four_fps_has_no_drift() {
        let total_samples = 26_460_000;
        let frames = total_frames(total_samples, 24);
        assert_eq!(frames, 14_400);
        assert_eq!(frame_sample_range(frames - 1, 24).end, total_samples);
    }

    #[test]
    fn total_frames_rounds_down() {
        // 1.5 frames at 30 fps is one complete frame.
        assert_eq!(total_frames(2_205, 30), 1);
        // Less than one frame of audio is zero frames.
        assert_eq!(total_frames(100, 30), 0);
        assert_eq!(total_frames(0, 30), 0);
    }

    #[test]
    fn frame_time_is_linear() {
        assert_eq!(frame_time_seconds(0, 30), 0.0);
        assert!((frame_time_seconds(30, 30) - 1.0).abs() < 1e-12);
        assert!((frame_time_seconds(15, 30) - 0.5).abs() < 1e-12);
    }
}
