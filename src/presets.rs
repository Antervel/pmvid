//! Preset discovery, ordering, and timed switching.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

/// Recursively find all `.milk` files under `dir`, sorted by path.
///
/// Returns an error if `dir` is not a directory or contains no presets.
pub fn scan(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        bail!(
            "preset directory '{}' does not exist or is not a directory",
            dir.display()
        );
    }

    let mut presets = Vec::new();
    walk(dir, &mut presets)
        .with_context(|| format!("scanning preset directory '{}'", dir.display()))?;
    presets.sort();

    if presets.is_empty() {
        bail!(
            "no .milk presets found in '{}'; pass --presets <DIR>",
            dir.display()
        );
    }

    Ok(presets)
}

/// Recursively collect `.milk` files under `dir` into `presets`.
///
/// Symlinks are skipped, so directory cycles cannot form.
fn walk(dir: &Path, presets: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            walk(&path, presets)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("milk"))
        {
            presets.push(path);
        }
    }
    Ok(())
}

/// Shuffle `presets` deterministically for a given seed.
///
/// The result is always a permutation of the input.
pub fn shuffle(mut presets: Vec<PathBuf>, seed: u64) -> Vec<PathBuf> {
    let mut rng = StdRng::seed_from_u64(seed);
    presets.shuffle(&mut rng);
    presets
}

/// Time-based preset switching state.
#[derive(Debug)]
pub struct PresetSchedule {
    presets: Vec<PathBuf>,
    frames_per_preset: u64,
    index: usize,
    next_switch_frame: u64,
}

impl PresetSchedule {
    /// Create a schedule that switches every `preset_duration_s` seconds.
    pub fn new(presets: Vec<PathBuf>, preset_duration_s: f64, fps: u32) -> Result<Self> {
        ensure!(!presets.is_empty(), "cannot schedule an empty preset list");
        ensure!(fps > 0, "fps must be greater than zero");
        ensure!(
            preset_duration_s.is_finite() && preset_duration_s > 0.0,
            "preset duration must be a positive number of seconds"
        );

        let frames_per_preset = (preset_duration_s * f64::from(fps)).round() as u64;
        ensure!(
            frames_per_preset > 0,
            "preset duration is shorter than one frame at {fps} fps"
        );

        Ok(Self {
            presets,
            frames_per_preset,
            index: 0,
            next_switch_frame: frames_per_preset,
        })
    }

    /// The preset that should currently be loaded.
    pub fn current(&self) -> &Path {
        &self.presets[self.index]
    }

    /// Number of frames each preset is shown for.
    pub fn frames_per_preset(&self) -> u64 {
        self.frames_per_preset
    }

    /// Advance if `frame` has reached the next switch point.
    ///
    /// Returns the new preset path when a switch happened, or `None` otherwise.
    /// Call this once per frame, before rendering.
    pub fn advance(&mut self, frame: u64) -> Option<&Path> {
        if frame < self.next_switch_frame {
            return None;
        }

        let next_switch_frame = self.next_switch_frame.checked_add(self.frames_per_preset)?;
        self.next_switch_frame = next_switch_frame;
        self.index = (self.index + 1) % self.presets.len();
        Some(self.current())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"[preset00]\n").unwrap();
    }

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn finds_milk_files_recursively_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("b.milk"));
        touch(&root.join("a.milk"));
        touch(&root.join("nested/c.milk"));
        touch(&root.join("nested/ignore.txt"));

        let found = scan(root).unwrap();
        let names: Vec<_> = found
            .iter()
            .map(|path| {
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        assert_eq!(names, vec!["a.milk", "b.milk", "nested/c.milk"]);
    }

    #[test]
    fn extension_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("upper.MILK"));

        let found = scan(dir.path()).unwrap();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn missing_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");

        let err = scan(&missing).unwrap_err();
        assert!(format!("{err:#}").contains("does not exist"));
    }

    #[test]
    fn empty_directory_is_an_error() {
        let dir = tempfile::tempdir().unwrap();

        let err = scan(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("no .milk presets found"));
    }

    #[test]
    fn files_that_are_not_presets_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("readme.txt"));
        touch(&dir.path().join("preset.milk"));

        let found = scan(dir.path()).unwrap();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn same_seed_produces_same_order() {
        let input = paths(&["a.milk", "b.milk", "c.milk", "d.milk", "e.milk"]);
        let first = shuffle(input.clone(), 42);
        let second = shuffle(input, 42);
        assert_eq!(first, second);
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut input = paths(&["a.milk", "b.milk", "c.milk", "d.milk"]);
        let shuffled = shuffle(input.clone(), 7);

        input.sort();
        let mut shuffled_sorted = shuffled.clone();
        shuffled_sorted.sort();

        assert_eq!(shuffled_sorted, input);
        assert_eq!(shuffled.len(), input.len());
    }

    #[test]
    fn different_seeds_usually_differ() {
        let input = paths(&[
            "a.milk", "b.milk", "c.milk", "d.milk", "e.milk", "f.milk", "g.milk", "h.milk",
        ]);
        let a = shuffle(input.clone(), 1);
        let b = shuffle(input, 2);
        assert_ne!(
            a, b,
            "two seeds produced the same order; check the RNG setup"
        );
    }

    #[test]
    fn empty_list_stays_empty() {
        assert!(shuffle(Vec::new(), 123).is_empty());
    }

    fn schedule(names: &[&str], seconds: f64, fps: u32) -> PresetSchedule {
        PresetSchedule::new(paths(names), seconds, fps).unwrap()
    }

    #[test]
    fn starts_on_the_first_preset() {
        let schedule = schedule(&["a.milk", "b.milk"], 20.0, 30);
        assert_eq!(schedule.current(), Path::new("a.milk"));
        assert_eq!(schedule.frames_per_preset(), 600);
    }

    #[test]
    fn switches_at_the_expected_frame() {
        let mut schedule = schedule(&["a.milk", "b.milk"], 20.0, 30);

        assert!(schedule.advance(0).is_none());
        assert!(schedule.advance(599).is_none());
        assert_eq!(schedule.advance(600), Some(Path::new("b.milk")));
        assert_eq!(schedule.current(), Path::new("b.milk"));

        assert!(schedule.advance(1_199).is_none());
        assert_eq!(schedule.advance(1_200), Some(Path::new("a.milk")));
    }

    #[test]
    fn wraps_around() {
        let mut schedule = schedule(&["a.milk", "b.milk", "c.milk"], 1.0, 30);
        assert_eq!(schedule.advance(30), Some(Path::new("b.milk")));
        assert_eq!(schedule.advance(60), Some(Path::new("c.milk")));
        assert_eq!(schedule.advance(90), Some(Path::new("a.milk")));
    }

    #[test]
    fn single_preset_repeats_itself() {
        let mut schedule = schedule(&["only.milk"], 1.0, 30);
        assert_eq!(schedule.advance(30), Some(Path::new("only.milk")));
        assert_eq!(schedule.advance(60), Some(Path::new("only.milk")));
    }

    #[test]
    fn duration_shorter_than_one_frame_is_rejected() {
        let err = PresetSchedule::new(paths(&["a.milk"]), 0.001, 30).unwrap_err();
        assert!(format!("{err:#}").contains("shorter than one frame"));
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        assert!(PresetSchedule::new(Vec::new(), 20.0, 30).is_err());
        assert!(PresetSchedule::new(paths(&["a.milk"]), 0.0, 30).is_err());
        assert!(PresetSchedule::new(paths(&["a.milk"]), f64::NAN, 30).is_err());
        assert!(PresetSchedule::new(paths(&["a.milk"]), 20.0, 0).is_err());
    }

    #[test]
    fn non_integer_frames_round_to_nearest() {
        // 2.5 s at 24 fps = 60 frames exactly.
        assert_eq!(schedule(&["a.milk"], 2.5, 24).frames_per_preset(), 60);
        // 2.55 s at 30 fps = 76.5 -> 77 frames.
        assert_eq!(schedule(&["a.milk"], 2.55, 30).frames_per_preset(), 77);
    }
    #[test]
    fn advance_returns_none_on_frame_overflow() {
        let mut schedule = schedule(&["a.milk"], 20.0, 30);
        schedule.frames_per_preset = u64::MAX;
        schedule.next_switch_frame = u64::MAX;
        assert!(schedule.advance(u64::MAX).is_none());
    }
}
