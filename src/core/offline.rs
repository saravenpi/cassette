//! Offline rendering and WAV bouncing (Phase 4.1).
//!
//! Rendering reuses the exact [`RenderCore`] the live engine uses, so a bounce
//! sounds like playback. The inner loop runs under `assert_no_alloc` in debug
//! builds, which is the same red-zone guarantee the audio thread relies on.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use assert_no_alloc::assert_no_alloc;

use crate::core::DEFAULT_SAMPLE_RATE;
use crate::core::render::{RenderCore, RtCommand};

/// Frames rendered per inner call.
const CHUNK_FRAMES: usize = 512;

/// Render `loops` passes of `song` to interleaved stereo samples.
pub fn render_song(song: &crate::models::Song, loops: usize, sample_rate: u32) -> Vec<f32> {
    if loops == 0 {
        return Vec::new();
    }
    let total_frames = song.frames_per_loop(sample_rate) as usize * loops;
    let mut output = vec![0.0f32; total_frames * 2];

    let mut core = RenderCore::new(song, sample_rate);
    core.apply(RtCommand::SetLooping { looping: false });
    core.apply(RtCommand::Play);

    let mut offset = 0usize;
    while offset < output.len() {
        let end = (offset + CHUNK_FRAMES * 2).min(output.len());
        let slice = &mut output[offset..end];
        // The DSP path must not allocate: this is the red-zone contract.
        assert_no_alloc(|| core.process(slice));
        offset = end;
    }
    output
}

/// Write interleaved stereo `f32` samples as a 16-bit PCM WAV file.
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = bits_per_sample as u32 / 8;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample;
    let block_align = channels * bytes_per_sample as u16;
    let data_len = (samples.len() as u32) * bytes_per_sample;

    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + data_len).to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?; // PCM
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;

    for &sample in samples {
        let quantized = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_all(&quantized.to_le_bytes())?;
    }
    writer.flush()?;
    Ok(())
}

/// Render and bounce `song` to a `.wav` file.
pub fn export_song(
    song: &crate::models::Song,
    path: &Path,
    loops: usize,
    sample_rate: u32,
) -> Result<()> {
    let samples = render_song(song, loops, sample_rate);
    write_wav(path, &samples, sample_rate)
}

/// Render at the default sample rate.
pub fn render_default(song: &crate::models::Song, loops: usize) -> Vec<f32> {
    render_song(song, loops, DEFAULT_SAMPLE_RATE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Song;

    #[test]
    fn rendered_length_matches_the_song() {
        let song = Song::demo();
        let loops = 2;
        let samples = render_song(&song, loops, DEFAULT_SAMPLE_RATE);
        let expected = song.frames_per_loop(DEFAULT_SAMPLE_RATE) as usize * loops * 2;
        assert_eq!(samples.len(), expected);
    }

    #[test]
    fn render_is_not_silent() {
        let song = Song::demo();
        let samples = render_song(&song, 1, DEFAULT_SAMPLE_RATE);
        let peak = samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(peak > 0.01, "bounce should contain audio, peak={peak}");
    }

    #[test]
    fn zero_loops_is_empty() {
        assert!(render_song(&Song::demo(), 0, DEFAULT_SAMPLE_RATE).is_empty());
    }

    #[test]
    fn wav_header_is_well_formed() {
        let samples = vec![0.0f32; 100 * 2];
        let dir = std::env::temp_dir();
        let path = dir.join("zboub_test_bounce.wav");
        write_wav(&path, &samples, DEFAULT_SAMPLE_RATE).expect("write wav");
        let bytes = std::fs::read(&path).expect("read back");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[36..40], b"data");
        // 44-byte header + 100 frames * 2 channels * 2 bytes.
        assert_eq!(bytes.len(), 44 + 400);
        let _ = std::fs::remove_file(&path);
    }
}
