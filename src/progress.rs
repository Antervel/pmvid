//! Optional progress reporting.

use indicatif::{ProgressBar, ProgressStyle};

/// A progress bar over video frames.
pub struct Progress {
    bar: ProgressBar,
}

impl Progress {
    /// Create a visible progress bar on stderr.
    pub fn new(total_frames: u64, fps: u32) -> Self {
        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        )
        .expect("static progress template is valid")
        .progress_chars("#>-");

        let bar = ProgressBar::new(total_frames);
        bar.set_style(style);
        bar.set_message(format!("{fps} fps"));
        Self { bar }
    }

    /// Create a no-op progress bar for non-interactive output.
    pub fn hidden() -> Self {
        Self {
            bar: ProgressBar::hidden(),
        }
    }

    /// Update the current frame. Safe to call with values above the total.
    pub fn set_frame(&self, frame: u64) {
        self.bar.set_position(frame);
    }

    /// Clear the bar when rendering finishes.
    pub fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_progress_does_not_panic() {
        let progress = Progress::hidden();
        progress.set_frame(0);
        progress.set_frame(100);
        progress.finish();
    }

    #[test]
    fn setting_a_frame_past_the_total_does_not_panic() {
        let progress = Progress::hidden();
        progress.set_frame(u64::MAX);
        progress.finish();
    }
}
