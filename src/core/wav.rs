//! A minimal WAV reader: PCM (8/16/24/32-bit) and IEEE float (32/64-bit).
//!
//! Samples are downmixed to mono, because a track plays one voice and the mixer
//! already places it in the stereo field. Reading happens on the control thread;
//! nothing here runs in the audio callback.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::{Hint, ProbeResult};

const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

pub const MAX_DECODED_FRAMES: usize = 10 * 60 * 48_000;

const FORMAT_PCM: u16 = 1;
const FORMAT_FLOAT: u16 = 3;
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;

const MAX_CHANNELS: u16 = 32;

const STREAMING_SIZE: usize = 0xFFFF_FFFF;

#[derive(Clone, Debug)]
pub struct Sample {
    pub frames: Vec<f32>,
    pub sample_rate: u32,
    pub source_channels: u16,
    pub source_frames: usize,
    pub truncated: bool,
    pub error: Option<String>,
}

struct Shared(Arc<Vec<u8>>);

impl AsRef<[u8]> for Shared {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn check_file(path: &Path) -> Result<()> {
    let meta = std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if !meta.is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if meta.len() > MAX_FILE_BYTES {
        bail!(
            "{} is {} bytes, past the {} byte ceiling",
            path.display(),
            meta.len(),
            MAX_FILE_BYTES
        );
    }
    Ok(())
}

fn hint_for(path: &Path) -> Hint {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }
    hint
}

pub fn decode(path: &Path, max_frames: usize) -> Result<Sample> {
    check_file(path)?;
    let hint = hint_for(path);
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    decode_stream(Box::new(file), &hint, max_frames)
        .or_else(|_| decode_owned(std::fs::read(path)?, max_frames))
        .with_context(|| format!("decoding {}", path.display()))
}

pub fn sample_rate(path: &Path) -> Result<u32> {
    check_file(path)?;
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let probed = probe(Box::new(file), &hint_for(path))?;
    probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .and_then(|t| t.codec_params.sample_rate)
        .context("missing sample rate")
}

struct PcmLengths {
    data: u32,
    total: u32,
    block_align: u16,
    byte_rate: u32,
}

fn pcm16_lengths(samples: usize, channels: u16, sample_rate: u32) -> Result<PcmLengths> {
    let data = u32::try_from((samples as u64).saturating_mul(2))
        .ok()
        .filter(|data| data.checked_add(36).is_some())
        .context("output is past the 4 GiB limit of a WAV file")?;
    let block_align = channels
        .checked_mul(2)
        .context("too many channels for a WAV file")?;
    let byte_rate = sample_rate
        .checked_mul(block_align as u32)
        .context("sample rate is too high for a WAV file")?;
    Ok(PcmLengths {
        data,
        total: data + 36,
        block_align,
        byte_rate,
    })
}

pub fn encode_pcm16(path: &Path, frames: &[f32], channels: u16, sample_rate: u32) -> Result<()> {
    let lengths = pcm16_lengths(frames.len(), channels, sample_rate)?;
    let bits: u16 = 16;

    let mut bytes = Vec::with_capacity(44 + lengths.data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&lengths.total.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&FORMAT_PCM.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&lengths.byte_rate.to_le_bytes());
    bytes.extend_from_slice(&lengths.block_align.to_le_bytes());
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&lengths.data.to_le_bytes());

    for &sample in frames {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * 32767.0).round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory for {}", path.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn decode_bytes(bytes: &[u8], max_frames: usize) -> Result<Sample> {
    decode_owned(bytes.to_vec(), max_frames)
}

fn decode_owned(bytes: Vec<u8>, max_frames: usize) -> Result<Sample> {
    let mut hint = Hint::new();
    hint.with_extension("wav");
    let shared = Arc::new(bytes);
    decode_stream(
        Box::new(Cursor::new(Shared(shared.clone()))),
        &hint,
        max_frames,
    )
    .or_else(|_| {
        let header = parse(&shared)?;
        let frames = header.decode(max_frames);
        Ok(Sample {
            frames,
            sample_rate: header.sample_rate,
            source_channels: header.channels,
            source_frames: header.frames,
            truncated: header.frames > max_frames,
            error: None,
        })
    })
}

fn probe(source: Box<dyn MediaSource>, hint: &Hint) -> Result<ProbeResult> {
    let mss = MediaSourceStream::new(source, Default::default());
    symphonia::default::get_probe()
        .format(
            hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("probing audio format")
}

fn is_end_of_stream(error: &SymphoniaError) -> bool {
    match error {
        SymphoniaError::IoError(e) => e.kind() == std::io::ErrorKind::UnexpectedEof,
        SymphoniaError::ResetRequired => true,
        _ => false,
    }
}

fn decode_stream(source: Box<dyn MediaSource>, hint: &Hint, max_frames: usize) -> Result<Sample> {
    let probed = probe(source, hint)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .context("no audio track found")?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .context("missing sample rate")?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("creating audio decoder")?;

    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut buf_frames = 0u64;
    let mut frames = Vec::new();
    let mut source_frames = 0;
    let mut source_channels = 0u16;
    let mut truncated = false;
    let mut error = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(e) if is_end_of_stream(&e) => break,
            Err(e) => {
                error = Some(format!("reading audio packet: {e}"));
                break;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        if frames.len() >= max_frames {
            truncated = true;
            break;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) if is_end_of_stream(&e) => break,
            Err(e) => {
                error = Some(format!("decoding audio packet: {e}"));
                break;
            }
        };
        let spec = *decoded.spec();
        let ch = spec.channels.count().max(1);
        source_channels = ch as u16;
        let capacity = decoded.capacity() as u64;
        if sample_buf.is_none() || capacity > buf_frames {
            sample_buf = Some(SampleBuffer::<f32>::new(capacity, spec));
            buf_frames = capacity;
        }
        let Some(buf) = &mut sample_buf else {
            continue;
        };
        buf.copy_interleaved_ref(decoded);
        let samples = buf.samples();
        let packet_frames = samples.len() / ch;
        source_frames += packet_frames;
        let room = max_frames - frames.len();
        if packet_frames > room {
            truncated = true;
        }
        frames.extend(
            samples
                .chunks_exact(ch)
                .take(room)
                .map(|frame| frame.iter().sum::<f32>() / ch as f32),
        );
    }

    if source_frames == 0 {
        match error {
            Some(error) => bail!("{error}"),
            None => bail!("audio stream contained no playable frames"),
        }
    }

    Ok(Sample {
        frames,
        sample_rate,
        source_channels,
        source_frames,
        truncated,
        error,
    })
}

/// The parts of the file the decoder needs.
struct Header<'a> {
    format: u16,
    bits: u16,
    channels: u16,
    sample_rate: u32,
    frames: usize,
    data: &'a [u8],
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
fn parse(bytes: &[u8]) -> Result<Header<'_>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("not a RIFF/WAVE file");
    }

    let mut format = None;
    let mut bits = 0u16;
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut data: Option<&[u8]> = None;

    let mut at = 12;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32_at(bytes, at + 4).context("chunk size")? as usize;
        let body = at + 8;
        let streaming = id == b"data" && (size == 0 || size == STREAMING_SIZE);
        let end = if streaming {
            bytes.len()
        } else {
            body.saturating_add(size).min(bytes.len())
        };

        match id {
            b"fmt " => {
                let raw = u16_at(bytes, body).context("fmt format")?;
                channels = u16_at(bytes, body + 2).context("fmt channels")?;
                sample_rate = u32_at(bytes, body + 4).context("fmt sample rate")?;
                bits = u16_at(bytes, body + 14).context("fmt bits per sample")?;
                format = Some(if raw == FORMAT_EXTENSIBLE {
                    u16_at(bytes, body + 24).context("extensible subformat")?
                } else {
                    raw
                });
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }

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

    if !matches!(
        (format, bits),
        (FORMAT_PCM, 8 | 16 | 24 | 32) | (FORMAT_FLOAT, 32 | 64)
    ) {
        bail!("{bits} bits per sample is not supported for WAV format {format}");
    }
    let bytes_per_frame = (bits as usize / 8) * channels as usize;
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

impl Header<'_> {
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
        let bytes = self.data;
        match (self.format, self.bits) {
            (FORMAT_FLOAT, 32) => u32_at(bytes, at).map_or(0.0, f32::from_bits),
            (FORMAT_FLOAT, 64) => bytes
                .get(at..at + 8)
                .map(|slice| {
                    let raw: [u8; 8] = slice.try_into().unwrap_or([0; 8]);
                    f64::from_le_bytes(raw) as f32
                })
                .unwrap_or(0.0),
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
        assert_eq!(sample.source_channels, 1);
        assert_eq!(sample.sample_rate, 44_100);
        assert!((sample.frames[0]).abs() < 1e-6);
        assert!((sample.frames[1] - 0.5).abs() < 1e-4);
        assert!((sample.frames[2] + 0.5).abs() < 1e-4);
    }

    #[test]
    fn stereo_is_downmixed_to_mono() {
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

        let sample =
            decode_bytes(&wav(FORMAT_PCM, 24, 1, 44_100, &[0xff, 0xff, 0x7f]), 10).expect("i24");
        assert!((sample.frames[0] - 1.0).abs() < 1e-6);

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
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("cassette-mix.wav");
        if !path.exists() {
            return;
        }
        let sample = decode(&path, 96_000).expect("the repo sample should decode");
        assert_eq!(sample.source_channels, 2);
        assert_eq!(sample.sample_rate, 44_100);
        assert_eq!(sample.frames.len(), 96_000);
        assert!(sample.truncated, "the file is longer than the ceiling");
        assert!(sample.frames.iter().any(|value| value.abs() > 1e-4));
    }

    #[test]
    fn mp3_flac_m4a_files_decode() {
        for ext in ["mp3", "flac", "m4a"] {
            let path = std::path::Path::new("/tmp").join(format!("test.{ext}"));
            if !path.exists() {
                continue;
            }
            let sample = decode(&path, 96_000).expect("should decode audio file");
            assert!(sample.source_channels >= 1);
            assert!(sample.sample_rate > 0);
            assert!(!sample.frames.is_empty());
        }
    }

    #[test]
    fn encoding_pcm16_survives_roundtrip() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("cassette_test_roundtrip.wav");
        let original_frames = vec![0.0f32, 0.5f32, -0.5f32, 0.25f32];
        encode_pcm16(&path, &original_frames, 1, 44_100).expect("encoding should succeed");
        let decoded = decode(&path, 100).expect("decoding should succeed");
        let _ = std::fs::remove_file(&path);
        assert_eq!(decoded.source_channels, 1);
        assert_eq!(decoded.sample_rate, 44_100);
        assert_eq!(decoded.frames.len(), 4);
        assert!((decoded.frames[0]).abs() < 1e-3);
        assert!((decoded.frames[1] - 0.5).abs() < 1e-3);
        assert!((decoded.frames[2] + 0.5).abs() < 1e-3);
    }

    fn declare_data_size(mut bytes: Vec<u8>, size: u32) -> Vec<u8> {
        bytes[40..44].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[test]
    fn a_data_size_past_the_end_is_clamped_to_the_bytes_present() {
        let bytes = declare_data_size(pcm16(&[1_000, 2_000, 3_000], 1), 1_000_000);
        let sample = decode_bytes(&bytes, 100).expect("should decode");
        assert_eq!(sample.frames.len(), 3);
        let header = parse(&bytes).expect("should parse");
        assert_eq!(header.frames, 3);
    }

    #[test]
    fn streaming_headers_read_to_the_end_of_the_file() {
        for size in [0, u32::MAX] {
            let bytes = declare_data_size(pcm16(&[1_000, 2_000, 3_000], 1), size);
            let header = parse(&bytes).expect("should parse");
            assert_eq!(header.frames, 3, "declared size {size}");
        }
    }

    #[test]
    fn an_unsupported_bit_depth_is_rejected_not_silenced() {
        let bytes = wav(FORMAT_PCM, 12, 1, 44_100, &[0; 8]);
        assert!(parse(&bytes).is_err());
        let bytes = wav(FORMAT_FLOAT, 16, 1, 44_100, &[0; 8]);
        assert!(parse(&bytes).is_err());
        assert!(decode_bytes(&bytes, 10).is_err());
    }

    #[test]
    fn the_channel_count_comes_from_the_decoded_buffer() {
        let sample = decode_bytes(&pcm16(&[100, 200, 300, 400], 2), 10).expect("should decode");
        assert_eq!(sample.source_channels, 2);
        assert_eq!(sample.frames.len(), 2);
    }

    #[test]
    fn a_clean_stream_reports_no_error() {
        let sample = decode_bytes(&pcm16(&[1, 2, 3], 1), 10).expect("should decode");
        assert!(sample.error.is_none());
    }

    #[test]
    fn only_real_end_of_stream_errors_end_quietly() {
        let eof = SymphoniaError::IoError(std::io::ErrorKind::UnexpectedEof.into());
        let broken = SymphoniaError::IoError(std::io::ErrorKind::PermissionDenied.into());
        assert!(is_end_of_stream(&eof));
        assert!(is_end_of_stream(&SymphoniaError::ResetRequired));
        assert!(!is_end_of_stream(&broken));
        assert!(!is_end_of_stream(&SymphoniaError::Unsupported("x")));
    }

    #[test]
    fn a_directory_is_not_a_sample() {
        assert!(decode(&std::env::temp_dir(), 10).is_err());
        assert!(sample_rate(&std::env::temp_dir()).is_err());
    }

    #[test]
    fn the_rate_can_be_probed_without_decoding() {
        let path = std::env::temp_dir().join("cassette_test_probe_rate.wav");
        std::fs::write(&path, pcm16(&[0, 1, 2], 1)).expect("should write");
        let rate = sample_rate(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(rate.expect("should probe"), 44_100);
    }

    #[test]
    fn output_past_four_gibibytes_is_an_error_not_an_overflow() {
        assert!(pcm16_lengths(usize::MAX / 4, 2, 44_100).is_err());
        assert!(pcm16_lengths(1 << 31, 2, 44_100).is_err());
        assert!(pcm16_lengths(16, u16::MAX, 44_100).is_err());
        let ok = pcm16_lengths(4, 2, 44_100).expect("a small file fits");
        assert_eq!((ok.data, ok.total), (8, 44));
    }
}
