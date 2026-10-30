# PRD: projectM Audio Visualizer Video Renderer (Rust)

**Status:** Draft v0.1
**Working name:** `pmvid`
**Date:** 2026-10-26

---

## 1. Overview

`pmvid` is a command-line tool written in Rust that takes an audio file (MP3 first, other formats later) and produces an MP4 video containing a [projectM](https://github.com/projectM-visualizer/projectm) (MilkDrop-compatible) visualization synchronized to the audio.

Rendering is **offline** (not realtime): audio is decoded up front, then fed to projectM in per-frame chunks, with each rendered frame piped into ffmpeg. This makes output deterministic and independent of machine speed.

## 2. Problem Statement

projectM's official frontends are built for realtime playback. There is no polished, general-purpose tool that turns "song.mp3" into "song.mp4" using projectM presets. The closest existing project (groovy.video) is C++, tested on a single Linux distro, and tied to Intel VAAPI hardware encoding.

## 3. Goals and Non-Goals

### Goals
- G1. Convert an MP3 into an MP4 with projectM visuals and the original audio muxed in.
- G2. Output is frame-accurate and reproducible: audio and visuals stay in sync regardless of render speed.
- G3. Run headless (no visible window), including on servers via EGL/Mesa software rendering.
- G4. Simple CLI with sensible defaults (1280x720, 30 fps, random presets).
- G5. Use the existing Rust bindings (`projectm-rs`) rather than writing new FFI where possible.

### Non-Goals (v1)
- Realtime preview or GUI.
- Hardware-accelerated video encoding (ffmpeg's default software x264 is fine).
- Lyrics, text overlays, or album art compositing.
- Windows/macOS support (Linux first; others are best effort later).
- A preset editor or preset authoring.

## 4. Users and Use Cases

| User | Use case |
|---|---|
| Musician / DJ | Create a visual video of a track to upload to YouTube or social media |
| Hobbyist developer | Batch-render videos from a folder of audio files |
| Server operator | Run rendering in a container/CI with no display |

## 5. Functional Requirements

| ID | Requirement | Priority |
|---|---|---|
| F1 | Decode MP3 to interleaved stereo f32 PCM at a fixed sample rate (44.1 kHz) | P0 |
| F2 | Create a headless OpenGL context and an offscreen framebuffer at the target resolution | P0 |
| F3 | Initialize projectM with window size, fps, and a preset directory | P0 |
| F4 | For each video frame, push the corresponding audio samples to projectM, render, and read back pixels | P0 |
| F5 | Pipe raw frames to an `ffmpeg` subprocess, muxing the original audio into an MP4 (H.264 + AAC) | P0 |
| F6 | CLI flags: input, output, width, height, fps, preset dir, preset duration, seed | P0 |
| F7 | Preset switching: timed (every N seconds); optionally on hard cuts / beats | P1 |
| F8 | Progress bar with ETA | P1 |
| F9 | Skip failed presets and continue (log a warning) | P1 |
| F10 | Support other input formats (FLAC, WAV, OGG, AAC) via `symphonia` | P2 |
| F11 | Fixed RNG seed so the same input produces the same preset sequence | P2 |
| F12 | Batch mode: directory in, directory out | P2 |

## 6. Non-Functional Requirements

- **Sync accuracy:** Audio and video drift must be under one frame over a 10-minute track. Use cumulative sample positions, not a rounded samples-per-frame constant.
- **Performance:** Target at least realtime on a typical machine with a GPU at 720p30. Software rendering (Mesa llvmpipe) may be slower but must still work.
- **Portability:** Linux x86_64 first. Document required system packages.
- **Robustness:** Clear error messages for missing ffmpeg, no GL context, unreadable audio, or an empty preset directory.
- **Licensing:** The tool is MIT/Apache-compatible. Note that projectM is LGPL and presets have their own licenses.
- **Container image:** The GitHub Actions workflow builds the `Dockerfile` after the fast and native jobs pass, and publishes `ghcr.io/<lowercase-owner>/pmvid` for pushes to the default branch and `v*` tags, for both the `linux/amd64` and the `linux/arm64` (variant `v8`) image.

## 7. CLI Specification

```
pmvid -i song.mp3 -o song.mp4 [options]

  -i, --input <FILE>           Input audio file (required)
  -o, --output <FILE>          Output video file (required)
  -W, --width <PX>             Default 1280
  -H, --height <PX>            Default 720
  -f, --fps <N>                Default 30
  -p, --presets <DIR>          Preset directory (default: ./presets)
      --preset-duration <SEC>  Seconds per preset (default 20)
      --beat-sensitivity <F>   Default 1.0
      --seed <N>               RNG seed for preset order
      --crf <N>                x264 quality (default 20)
      --ffmpeg <PATH>          ffmpeg binary (default: ffmpeg on PATH)
```

## 8. Architecture

```
 song.mp3
    │
    ▼
┌──────────────┐   interleaved f32 PCM   ┌───────────────────────────┐
│ Audio decode │ ──────────────────────▶ │       Main render loop    │
│ (symphonia)  │                         │                           │
└──────────────┘                         │  per frame:               │
                                         │   1. pcm_add_float(chunk) │
┌──────────────┐   GL context + FBO      │   2. render_frame()       │
│ Headless GL  │ ──────────────────────▶ │   3. glReadPixels         │
│ (EGL / SDL)  │                         │   4. write to ffmpeg      │
└──────────────┘                         └─────────────┬─────────────┘
                                                       │ raw RGBA
┌──────────────┐                                       ▼
│  projectM    │ ◀── used by loop        ┌───────────────────────────┐
│ (projectm-rs)│                         │ ffmpeg subprocess (stdin) │
└──────────────┘                         │ + original audio → MP4    │
                                         └───────────────────────────┘
```

### Modules

| Module | Responsibility |
|---|---|
| `audio.rs` | Decode and resample input to 44.1 kHz stereo f32 |
| `gl_ctx.rs` | Create headless GL context, FBO, read pixels |
| `encoder.rs` | Spawn ffmpeg, write frames to stdin, finalize |
| `presets.rs` | Scan preset directory, ordering, switching schedule |
| `main.rs` | CLI parsing (`clap`), orchestration, progress |

### Key Dependencies

| Crate / tool | Purpose |
|---|---|
| `projectm` / `projectm-sys` (projectm-rs) | projectM bindings |
| `symphonia` | MP3 (and other) decoding |
| `khronos-egl` + `gl` (or `glutin` / `sdl2` with hidden window) | Headless GL context and GL calls |
| `clap` | CLI |
| `indicatif` | Progress bar |
| `anyhow` | Error handling |
| `ffmpeg` (external binary) | Video encode and audio mux |

System: CMake, C++ toolchain, OpenGL dev libs, Mesa EGL (for headless), ffmpeg.

## 9. Main Loop Sketch

> This is a **sketch**, not compiling code. Exact projectm-rs method names and signatures differ between crate versions (1.x uses a handle with associated functions; newer versions expose a `ProjectM` struct with `&self` methods). Verify against the version you pin. `HeadlessGl` is a thin wrapper you write around EGL (or a hidden SDL window).

```rust
use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

const SAMPLE_RATE: u64 = 44_100;
const CHANNELS: usize = 2;
// projectM's PCM buffer is small; feed it in chunks no larger than this.
// (Query projectm's pcm max-samples at runtime and use that value.)
const PCM_CHUNK: usize = 512;

struct Config {
    input: String,
    output: String,
    width: u32,
    height: u32,
    fps: u64,
    preset_dir: String,
    preset_duration_s: f64,
}

fn main() -> Result<()> {
    let cfg = parse_args(); // clap

    // 1. Decode the whole track up front: interleaved f32, 44.1 kHz stereo.
    let pcm: Vec<f32> = audio::decode_to_f32_stereo(&cfg.input, SAMPLE_RATE as u32)
        .context("decoding audio")?;
    let total_samples_per_ch = (pcm.len() / CHANNELS) as u64;
    let total_frames = total_samples_per_ch * cfg.fps / SAMPLE_RATE;

    // 2. Headless GL context + FBO at target resolution. Context is made current here.
    let gl = HeadlessGl::new(cfg.width, cfg.height).context("creating GL context")?;

    // 3. projectM setup (must happen AFTER the GL context is current).
    let pm = ProjectM::create();
    pm.set_window_size(cfg.width as usize, cfg.height as usize);
    pm.set_fps(cfg.fps as i32);
    pm.set_beat_sensitivity(1.0);
    // pm.set_texture_search_paths(&[format!("{}/textures", cfg.preset_dir)], 1);

    let presets = presets::scan(&cfg.preset_dir)?; // Vec<PathBuf>, shuffled w/ seed
    anyhow::ensure!(!presets.is_empty(), "no presets found in {}", cfg.preset_dir);
    let frames_per_preset = (cfg.preset_duration_s * cfg.fps as f64) as u64;
    let mut preset_idx = 0usize;
    pm.load_preset_file(presets[preset_idx].to_str().unwrap(), false);

    // 4. Spawn ffmpeg: raw RGBA frames on stdin + original audio file -> MP4.
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-y", "-f", "rawvideo", "-pix_fmt", "rgba",
               "-s", &format!("{}x{}", cfg.width, cfg.height),
               "-r", &cfg.fps.to_string(), "-i", "-",
               "-i", &cfg.input,
               "-map", "0:v", "-map", "1:a",
               "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20",
               "-c:a", "aac", "-b:a", "192k",
               "-shortest", &cfg.output])
        .stdin(Stdio::piped())
        .spawn()
        .context("spawning ffmpeg (is it installed?)")?;
    let mut ffmpeg_in = ffmpeg.stdin.take().unwrap();

    // 5. Main loop: one iteration per video frame.
    let mut frame_buf = vec![0u8; (cfg.width * cfg.height * 4) as usize];
    let mut flipped = vec![0u8; frame_buf.len()];

    for frame in 0..total_frames {
        // Cumulative sample positions avoid drift when SAMPLE_RATE % fps != 0.
        let start = (frame * SAMPLE_RATE / cfg.fps) as usize;
        let end   = ((frame + 1) * SAMPLE_RATE / cfg.fps) as usize;
        let window = &pcm[start * CHANNELS..end * CHANNELS];

        // Feed the audio belonging to this frame (in small chunks).
        for chunk in window.chunks(PCM_CHUNK * CHANNELS) {
            pm.pcm_add_float(chunk.to_vec(), CHANNELS as u32);
        }

        // Timed preset switching.
        if frame > 0 && frame % frames_per_preset == 0 {
            preset_idx = (preset_idx + 1) % presets.len();
            pm.load_preset_file(presets[preset_idx].to_str().unwrap(), true); // soft cut
        }

        // Render into our FBO and read it back.
        gl.bind_fbo();
        pm.render_frame();
        gl.read_pixels_rgba(&mut frame_buf);   // glReadPixels

        // OpenGL origin is bottom-left; flip rows (or use ffmpeg `-vf vflip`).
        flip_vertical(&frame_buf, &mut flipped, cfg.width, cfg.height);

        ffmpeg_in.write_all(&flipped).context("writing frame to ffmpeg")?;

        if frame % cfg.fps == 0 {
            progress(frame, total_frames);
        }
    }

    // 6. Finalize.
    drop(ffmpeg_in); // closes stdin so ffmpeg can finish
    let status = ffmpeg.wait()?;
    anyhow::ensure!(status.success(), "ffmpeg exited with {status}");
    Ok(())
}

fn flip_vertical(src: &[u8], dst: &mut [u8], w: u32, h: u32) {
    let row = (w * 4) as usize;
    for y in 0..h as usize {
        let s = (h as usize - 1 - y) * row;
        dst[y * row..(y + 1) * row].copy_from_slice(&src[s..s + row]);
    }
}
```

### Loop notes
- **Why cumulative positions:** at 44100 Hz and 30 fps there are exactly 1470 samples per frame, but at 24 or 29.97 fps there isn't an integer count; computing `frame * SR / fps` each time keeps drift at zero.
- **Chunked PCM:** projectM's internal PCM buffer holds only a few hundred samples, so a full frame's worth (1470) cannot go in a single call. Feeding in chunks, or just the final chunk of each frame window, are both viable; test which looks better.
- **Context ordering:** the GL context must be current before creating projectM, and on the same thread for the whole run.
- **Flip:** doing `-vf vflip` in ffmpeg avoids the CPU copy and is a simple optimization.

## 10. Technical Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| **Wall-clock time inside projectM** makes animation speed depend on render speed in offline mode | High | Spike early: check whether the pinned projectM version allows a custom/fixed frame time; if not, patch or work around (e.g. pace rendering, or contribute upstream) |
| `projectm-rs` crate is outdated relative to projectM 4.x, or missing needed functions | High | Pin the version that matches; fall back to `projectm-sys` raw bindings for missing calls |
| Headless GL context creation is flaky across drivers | Medium | Support both EGL surfaceless/pbuffer and a hidden SDL window; document `xvfb-run` and Mesa llvmpipe fallback |
| Default framebuffer assumptions in projectM | Medium | Render into explicit FBO; verify projectM restores/uses bound framebuffer correctly |
| Readback (`glReadPixels`) is a bottleneck at high resolution | Medium | Use PBOs with double buffering (P2), or lower the default resolution |
| Preset compile failures / crashes | Low | Use preset-failed callback, skip and continue |
| Build complexity (CMake, system libs) | Medium | Document dependencies; consider a Dockerfile |

## 11. Milestones

| # | Milestone | Deliverable |
|---|---|---|
| M0 | **Spike** (1 day) | Hidden GL context, one projectM preset renders to PNG; confirm API surface and timing behavior |
| M1 | Audio to video, minimal | Decode MP3, fixed preset, piped to ffmpeg, muxed MP4 plays in sync |
| M2 | CLI + presets | clap flags, preset scanning, timed switching with soft cuts, progress bar |
| M3 | Hardening | Error handling, failed-preset skip, seeded RNG, README with install steps |
| M4 | Extras | Other audio formats, batch mode, PBO readback, Dockerfile |

## 12. Testing and Acceptance Criteria

- **Sync test:** render a track with a click or beat every second; verify in a video player that flashes align with clicks across a 10-minute file.
- **Determinism:** the same input and `--seed` produce visually identical output across two runs.
- **Headless test:** runs successfully under `xvfb-run` and in a Docker container without a display.
- **Resolution test:** 720p, 1080p, and a non-16:9 size all produce valid output.
- **Failure test:** missing ffmpeg, empty preset dir, corrupt MP3 each produce a clear error and non-zero exit code.
- **Acceptance:** a 4-minute MP3 yields a playable MP4 (H.264/AAC) with visuals reacting to the audio and no noticeable drift.

## 13. Open Questions

1. Which projectM version/crate version will be pinned, and does it expose a controllable frame time?
2. EGL (pure headless) or hidden SDL window as the default GL backend?
3. Should presets be bundled (license check) or always user-supplied?
4. Is hardware encoding (NVENC/VAAPI) worth exposing as a flag in v1.x?
5. Is the preset schedule time-based only, or should it react to beat/hard-cut events?
