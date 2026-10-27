//! `pmvid` renders a projectM visualization for an audio file into an MP4.
//!
//! The pure-Rust modules are always available. The native renderer lives behind the `projectm`
//! feature.

/// Sample rate used for projectM analysis.
pub const SAMPLE_RATE: u32 = 44_100;

/// Number of interleaved channels fed to projectM.
pub const CHANNELS: usize = 2;

pub mod audio;
pub mod cli;
pub mod encoder;
pub mod presets;
pub mod progress;
pub mod timebase;

#[cfg(test)]
mod tests {
    #[test]
    fn constants_are_sane() {
        assert_eq!(super::SAMPLE_RATE, 44_100);
        assert_eq!(super::CHANNELS, 2);
    }
}
