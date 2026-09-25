//! A minimal WAV reader: PCM (8/16/24/32-bit) and IEEE float (32/64-bit).
//!
//! Samples are downmixed to mono, because a track plays one voice and the mixer
//! already places it in the stereo field. Reading happens on the control thread;
//! nothing here runs in the audio callback.

use std::path::Path;

use anyhow::{Context, Result, bail};

/// Largest file the reader will look at, so a stray path cannot exhaust memory.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

const FORMAT_PCM: u16 = 1;
const FORMAT_FLOAT: u16 = 3;
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// Highest channel count the reader will downmix.
const MAX_CHANNELS: u16 = 32;

/// A decoded audio file: mono frames at the file's own rate.
#[derive(Clone, Debug)]
pub struct Sample {
    /// Mono frames, normalised to `[-1, 1]`.
    pub frames: Vec<f32>,
    /// The rate the frames are at, which need not match the engine's.
    pub sample_rate: u32,
    /// Channels the file carried, before the downmix.
    pub channels: u16,
    /// Frames the file held, before any truncation.
    pub source_frames: usize,
    /// Whether the file held more frames than `max_frames`.
    pub truncated: bool,
}

/// Read and decode a WAV file, keeping at most `max_frames` frames.
pub fn decode(path: &Path, max_frames: usize) -> Result<Sample> {
    let meta = std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if meta.len() > MAX_FILE_BYTES {
        bail!(
            "{} is {} bytes, past the {} byte ceiling",
            path.display(),
            meta.len(),
            MAX_FILE_BYTES
        );
    }
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    decode_bytes(&bytes, max_frames).with_context(|| format!("decoding {}", path.display()))
}

/// Decode WAV bytes, keeping at most `max_frames` frames.
pub fn decode_bytes(bytes: &[u8], max_frames: usize) -> Result<Sample> {
    let header = parse(bytes)?;
    let frames = header.decode(max_frames);
    Ok(Sample {
        frames,
        sample_rate: header.sample_rate,
        channels: header.channels,
        source_frames: header.frames,
        truncated: header.frames > max_frames,
    })
}

/// The parts of the file the decoder needs.
struct Header {
    format: u16,
    bits: u16,
    channels: u16,
    sample_rate: u32,
    frames: usize,
    data: Vec<u8>,
}

/// Little-endian `u16` at `at`, or `None` when the buffer is too short.
fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

/// Little-endian `u32` at `at`, or `None` when the buffer is too short.
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// Walk the RIFF chunks and pull out `fmt ` and `data`.
fn parse(bytes: &[u8]) -> Result<Header> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("not a RIFF/WAVE file");
    }

    let mut format = None;
    let mut bits = 0u16;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut data: Option<Vec<u8>> = None;

    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4).context("chunk size")? as usize;
        let body = at + 8;
        let end = body
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .context("chunk runs past the end of the file")?;

        match id {
            b"fmt " => {
                let raw = u16_at(bytes, body).context("fmt format")?;
                channels = u16_at(bytes, body + 2).context("fmt channels")?;
                sample_rate = u32_at(bytes, body + 4).context("fmt sample rate")?;
                bits = u16_at(bytes, body + 14).context("fmt bits per sample")?;
                // WAVE_FORMAT_EXTENSIBLE hides the real format in a subformat GUID.
                format = Some(if raw == FORMAT_EXTENSIBLE {
                    u16_at(bytes, body + 24).context("extensible subformat")?
                } else {
                    raw
                });
            }
            b"data" => data = Some(bytes[body..end].to_vec()),
            _ => {}
        }

        // Chunks are word-aligned: an odd size carries one byte of padding.
        at = end + (size & 1);
    }

    let format = format.context("no fmt chunk")?;
    let data = data.context("no data chunk")?;
    if channels == 0 || channels > MAX_CHANNELS {
        bail!("{channels} channels is outside 1..={MAX_CHANNELS}");
    }
    if sample_rate == 0 {
        bail!("a sample rate of 0 is not a rate");
    }
    if format != FORMAT_PCM && format != FORMAT_FLOAT {
        bail!("unsupported WAV format {format}");
    }

    let bytes_per_frame = (bits as usize / 8) * channels as usize;
    if bytes_per_frame == 0 {
        bail!("{bits} bits per sample is not a depth");
    }
    let frames = data.len() / bytes_per_frame;

    Ok(Header {
        format,
        bits,
        channels,
        sample_rate,
        frames,
        data,
    })
}

impl Header {
    /// Downmix the interleaved frames to mono.
    fn decode(&self, max_frames: usize) -> Vec<f32> {
        let channels = self.channels as usize;
        let width = self.bits as usize / 8;
        let frames = self.frames.min(max_frames);
        let mut out = Vec::with_capacity(frames);
        for frame in 0..frames {
            let base = frame * channels * width;
            let mut sum = 0.0f32;
            for channel in 0..channels {
                sum += self.value_at(base + channel * width);
            }
            out.push(sum / channels as f32);
        }
        out
    }

    /// One channel's sample, normalised to `[-1, 1]`.
    fn value_at(&self, at: usize) -> f32 {
        let bytes = &self.data;
        match (self.format, self.bits) {
            (FORMAT_FLOAT, 32) => u32_at(bytes, at).map_or(0.0, f32::from_bits),
            (FORMAT_FLOAT, 64) => bytes
                .get(at..at + 8)
                .map(|slice| {
                    let raw: [u8; 8] = slice.try_into().unwrap_or([0; 8]);
                    f64::from_le_bytes(raw) as f32
                })
                .unwrap_or(0.0),
            // 8-bit PCM is unsigned, unlike every deeper depth.
            (FORMAT_PCM, 8) => bytes.get(at).map_or(0.0, |byte| *byte as f32 / 128.0 - 1.0),
            (FORMAT_PCM, 16) => i16_at(bytes, at).map_or(0.0, |raw| raw as f32 / 32_768.0),
            (FORMAT_PCM, 24) => i24_at(bytes, at).map_or(0.0, |raw| raw as f32 / 8_388_608.0),
            (FORMAT_PCM, 32) => {
                let raw = u32_at(bytes, at).unwrap_or(0) as i32;
                raw as f32 / 2_147_483_648.0
            }
            _ => 0.0,
        }
    }
}

/// Little-endian `i16` at `at`, or `None` when the buffer is too short.
fn i16_at(bytes: &[u8], at: usize) -> Option<i16> {
    u16_at(bytes, at).map(|raw| raw as i16)
}

/// Little-endian 24-bit signed integer at `at`, sign-extended.
fn i24_at(bytes: &[u8], at: usize) -> Option<i32> {
    let slice = bytes.get(at..at + 3)?;
    let raw = i32::from_le_bytes([slice[0], slice[1], slice[2], 0]);
    Some(raw.wrapping_shl(8) >> 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a WAV in memory around the given format block and sample data.
    fn wav(format: u16, bits: u16, channels: u16, rate: u32, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&format.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        let block_align = channels * (bits / 8);
        out.extend_from_slice(&(rate * block_align as u32).to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        out
    }

    fn pcm16(samples: &[i16], channels: u16) -> Vec<u8> {
        let mut data = Vec::new();
        for sample in samples {
            data.extend_from_slice(&sample.to_le_bytes());
        }
        wav(FORMAT_PCM, 16, channels, 44_100, &data)
    }

    #[test]
    fn a_mono_pcm_file_decodes_to_its_own_values() {
        let bytes = pcm16(&[0, 16_384, -16_384], 1);
        let sample = decode_bytes(&bytes, 1_000).expect("should decode");
        assert_eq!(sample.frames.len(), 3);
        assert_eq!(sample.channels, 1);
        assert_eq!(sample.sample_rate, 44_100);
        assert!((sample.frames[0]).abs() < 1e-6);
        assert!((sample.frames[1] - 0.5).abs() < 1e-4);
        assert!((sample.frames[2] + 0.5).abs() < 1e-4);
    }

    #[test]
    fn stereo_is_downmixed_to_mono() {
        // Full left, silence right: the downmix is half amplitude, not full.
        let bytes = pcm16(&[32_767, 0, 32_767, 0], 2);
        let sample = decode_bytes(&bytes, 1_000).expect("should decode");
        assert_eq!(sample.frames.len(), 2);
        assert!((sample.frames[0] - 0.5).abs() < 1e-3);
    }

    #[test]
    fn the_frame_ceiling_truncates_and_is_reported() {
        let bytes = pcm16(&[0, 1_000, 2_000, 3_000], 1);
        let sample = decode_bytes(&bytes, 2).expect("should decode");
        assert_eq!(sample.frames.len(), 2);
        assert_eq!(sample.source_frames, 4);
        assert!(sample.truncated);
    }

    #[test]
    fn an_exact_fit_is_not_reported_as_truncated() {
        let bytes = pcm16(&[0, 1_000], 1);
        let sample = decode_bytes(&bytes, 2).expect("should decode");
        assert!(!sample.truncated);
    }

    #[test]
    fn float_and_deep_formats_decode() {
        let mut floats = Vec::new();
        for value in [0.25f32, -0.5] {
            floats.extend_from_slice(&value.to_le_bytes());
        }
        let sample = decode_bytes(&wav(FORMAT_FLOAT, 32, 1, 48_000, &floats), 10).expect("float");
        assert!((sample.frames[0] - 0.25).abs() < 1e-6);
        assert!((sample.frames[1] + 0.5).abs() < 1e-6);

        // 24-bit: 0x7FFFFF is the positive ceiling.
        let sample =
            decode_bytes(&wav(FORMAT_PCM, 24, 1, 44_100, &[0xff, 0xff, 0x7f]), 10).expect("i24");
        assert!((sample.frames[0] - 1.0).abs() < 1e-6);

        // 8-bit PCM is unsigned: 0x80 is silence, 0xff is positive.
        let sample = decode_bytes(&wav(FORMAT_PCM, 8, 1, 44_100, &[0x80, 0xff]), 10).expect("u8");
        assert!(sample.frames[0].abs() < 1e-6);
        assert!((sample.frames[1] - 0.992).abs() < 0.01);
    }

    #[test]
    fn an_extensible_file_reports_its_real_format() {
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&FORMAT_EXTENSIBLE.to_le_bytes());
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&44_100u32.to_le_bytes());
        fmt.extend_from_slice(&88_200u32.to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        fmt.extend_from_slice(&22u16.to_le_bytes()); // cbSize
        fmt.extend_from_slice(&16u16.to_le_bytes()); // valid bits
        fmt.extend_from_slice(&0u32.to_le_bytes()); // channel mask
        fmt.extend_from_slice(&FORMAT_PCM.to_le_bytes());
        fmt.extend_from_slice(&[0; 14]); // the rest of the GUID

        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(4u32 + 8 + fmt.len() as u32 + 8 + 4).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        out.extend_from_slice(&fmt);
        out.extend_from_slice(b"data");
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&16_384i16.to_le_bytes());
        out.extend_from_slice(&16_384i16.to_le_bytes());

        let sample = decode_bytes(&out, 10).expect("extensible");
        assert!((sample.frames[0] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn rubbish_is_rejected_rather_than_guessed_at() {
        assert!(decode_bytes(b"not a wave file at all", 10).is_err());
        assert!(decode_bytes(&pcm16(&[0], 1)[..10], 10).is_err());
        let mut no_data = pcm16(&[0], 1);
        no_data.truncate(36);
        assert!(decode_bytes(&no_data, 10).is_err());
    }

    #[test]
    fn the_repo_sample_decodes() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("zboub69-mix.wav");
        if !path.exists() {
            return;
        }
        let sample = decode(&path, 96_000).expect("the repo sample should decode");
        assert_eq!(sample.channels, 2);
        assert_eq!(sample.sample_rate, 44_100);
        assert_eq!(sample.frames.len(), 96_000);
        assert!(sample.truncated, "the file is longer than the ceiling");
        assert!(sample.frames.iter().any(|value| value.abs() > 1e-4));
    }
}
