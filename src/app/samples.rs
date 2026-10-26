//! Audio samples: decoding a file, giving it a slot on the audio side, and
//! turning it into a block on a lane.
//!
//! Decoding is a control-thread job. The frames then travel down an `rtrb` ring
//! into memory the audio thread allocated before the stream started, so a load
//! never blocks the render path.

use std::path::Path;
use std::sync::Arc;

use rtrb::Producer;

use super::App;
use crate::core::MAX_BLOCKS;
use crate::core::sample::{MAX_SAMPLE_SLOTS, SampleMsg};
use crate::core::wav::{self, Sample};
use crate::models::{Block, TrackKind};
use crate::ui::view::Cursor;

/// Buckets [`peaks`] reduces a waveform to by default.
pub const WAVEFORM_BUCKETS: usize = 256;

impl App {
    /// Load a WAV onto a track as an audio block.
    ///
    /// A dropped sample is what makes a track an audio track, so a MIDI track is
    /// flipped first. A load the ring refuses is dropped whole rather than
    /// blocking: the slot goes back and the status says so.
    pub(crate) fn import_sample(&mut self, track: usize, path: String, start_beats: f32) {
        if self.song.tracks.is_empty() {
            return;
        }
        let track_index = track.min(self.song.tracks.len() - 1);
        if self.song.tracks[track_index].blocks.len() >= MAX_BLOCKS {
            self.status = format!(
                "track {} is already at {MAX_BLOCKS} blocks",
                track_index + 1
            );
            return;
        }
        if !self.samples.contains_key(&path) && self.producer.is_none() {
            self.status = "the audio thread takes no samples".into();
            return;
        }
        let loaded = match self.ensure_loaded(&path) {
            Ok(loaded) => loaded,
            Err(message) => {
                self.status = message;
                return;
            }
        };
        let length_beats =
            (loaded.frames as f64 / loaded.rate as f64 / (60.0 / self.song.bpm() as f64)) as f32;
        let name = Path::new(&path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Sample")
            .to_string();
        let flipped = self.song.tracks[track_index].kind != TrackKind::Audio;
        self.song.tracks[track_index].kind = TrackKind::Audio;
        let index = self.song.tracks[track_index].insert_block(Block::new(
            name.clone(),
            TrackKind::Audio,
            start_beats,
            length_beats,
            path,
        ));
        self.cursor = Cursor {
            track: track_index,
            block: index,
        };
        self.push_mirror(track_index);
        self.status = if flipped {
            format!("loaded {name}: track {} is now AUDIO", track_index + 1)
        } else {
            format!("loaded {name} into track {}", track_index + 1)
        };
        if let Some(note) = loaded.note {
            self.status = format!("{}; {note}", self.status);
        }
    }

    fn ensure_loaded(&mut self, path: &str) -> Result<Loaded, String> {
        if let Some(&slot) = self.samples.get(path) {
            let rate = wav::sample_rate(Path::new(path))
                .map_err(|error| format!("cannot read {path}: {error:#}"))?;
            let frames = self
                .loaded_samples
                .get(&slot)
                .map_or(0, |frames| frames.len());
            return Ok(Loaded {
                slot,
                frames,
                rate,
                note: None,
            });
        }
        let slot = self
            .slot_for(path)
            .ok_or_else(|| format!("all {MAX_SAMPLE_SLOTS} sample slots are busy"))?;
        let sample = wav::decode(Path::new(path), wav::MAX_DECODED_FRAMES)
            .map_err(|error| format!("cannot read {path}: {error:#}"))?;
        let note = decode_note(&sample);
        let rate = sample.sample_rate;
        let frames: Arc<[f32]> = Arc::from(sample.frames);
        let count = frames.len();
        if let Some(producer) = self.producer.as_mut()
            && !push_load(producer, slot, rate, frames.clone())
        {
            let _ = producer.push(SampleMsg::Free { slot });
            return Err(format!(
                "{path} dropped: another load is still in flight, try again in a moment"
            ));
        }
        self.loaded_samples.insert(slot, frames);
        self.samples.insert(path.to_string(), slot);
        self.send(crate::core::render::RtCommand::SetSampleRate {
            slot,
            rate: rate as f32,
        });
        Ok(Loaded {
            slot,
            frames: count,
            rate,
            note,
        })
    }

    fn slot_for(&self, path: &str) -> Option<u8> {
        if let Some(slot) = self.samples.get(path) {
            return Some(*slot);
        }
        (0..MAX_SAMPLE_SLOTS as u8).find(|slot| !self.samples.values().any(|taken| taken == slot))
    }

    pub(crate) fn load_sample_source(&mut self, path: &str) -> Option<u8> {
        if let Some(&slot) = self.samples.get(path) {
            return Some(slot);
        }
        let loaded = self.ensure_loaded(path).ok()?;
        if let Some(note) = loaded.note {
            self.status = format!("{path}: {note}");
        }
        Some(loaded.slot)
    }
}

struct Loaded {
    slot: u8,
    frames: usize,
    rate: u32,
    note: Option<String>,
}

fn decode_note(sample: &Sample) -> Option<String> {
    let mut notes = Vec::new();
    if sample.truncated {
        notes.push(format!(
            "truncated to the first {} frames",
            wav::MAX_DECODED_FRAMES
        ));
    }
    if let Some(error) = &sample.error {
        notes.push(format!("stopped early: {error}"));
    }
    (!notes.is_empty()).then(|| notes.join(", "))
}

fn push_load(producer: &mut Producer<SampleMsg>, slot: u8, rate: u32, frames: Arc<[f32]>) -> bool {
    producer
        .push(SampleMsg::Load {
            slot,
            frames,
            rate: rate as f32,
        })
        .is_ok()
}

/// Reduce mono frames to `buckets` peak values in `0..=1`, for drawing an audio
/// block's waveform.
pub fn peaks(frames: &[f32], buckets: usize) -> Vec<f32> {
    if frames.is_empty() || buckets == 0 {
        return Vec::new();
    }
    (0..buckets)
        .map(|bucket| {
            let start = bucket * frames.len() / buckets;
            let end = ((bucket + 1) * frames.len() / buckets)
                .max(start + 1)
                .min(frames.len());
            frames[start..end]
                .iter()
                .fold(0.0f32, |peak, frame| peak.max(frame.abs()))
                .min(1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;

    /// A 16-bit mono WAV holding `frames` frames of a low square wave.
    fn write_wav(path: &Path, frames: usize) {
        let mut data = Vec::new();
        for frame in 0..frames {
            let value: i16 = if frame % 2 == 0 { 8_192 } else { -8_192 };
            data.extend_from_slice(&value.to_le_bytes());
        }
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        file.extend_from_slice(b"WAVEfmt ");
        file.extend_from_slice(&16u32.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&44_100u32.to_le_bytes());
        file.extend_from_slice(&88_200u32.to_le_bytes());
        file.extend_from_slice(&2u16.to_le_bytes());
        file.extend_from_slice(&16u16.to_le_bytes());
        file.extend_from_slice(b"data");
        file.extend_from_slice(&(data.len() as u32).to_le_bytes());
        file.extend_from_slice(&data);
        std::fs::write(path, file).expect("the test wav should be writable");
    }

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    /// A sample path, written on demand, that the test removes afterwards.
    struct TempWav(std::path::PathBuf);

    impl TempWav {
        fn new(name: &str, frames: usize) -> Self {
            let path = std::env::temp_dir().join(format!("cassette-{name}.wav"));
            write_wav(&path, frames);
            Self(path)
        }

        fn path(&self) -> String {
            self.0.to_string_lossy().into_owned()
        }
    }

    impl Drop for TempWav {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn a_dropped_sample_becomes_an_audio_block() {
        let wav = TempWav::new("drop", 22_050);
        let mut app = headless_app();
        app.handle(Event::ImportSample {
            track: 2,
            path: wav.path(),
            start_beats: 4.0,
        });
        let block = &app.song.tracks[2].blocks[0];
        assert_eq!(block.kind, TrackKind::Audio);
        assert_eq!(block.source, wav.path());
        assert_eq!(block.name, "cassette-drop");
        assert_eq!(block.start_beats, 4.0);
        assert!(
            (block.length_beats - 1.0).abs() < 1e-3,
            "half a second at 120 bpm is one beat, got {}",
            block.length_beats
        );
        assert!(app.samples.contains_key(&wav.path()));
        assert!(app.status.contains("loaded"), "status: {}", app.status);
    }

    #[test]
    fn a_drop_makes_a_midi_track_an_audio_track() {
        let wav = TempWav::new("flip", 22_050);
        let mut app = headless_app();
        assert_eq!(app.song.tracks[0].kind, TrackKind::Midi);
        app.handle(Event::ImportSample {
            track: 0,
            path: wav.path(),
            start_beats: 0.0,
        });
        assert_eq!(app.song.tracks[0].kind, TrackKind::Audio);
        assert!(app.status.contains("AUDIO"), "status: {}", app.status);
    }

    #[test]
    fn dropping_the_same_file_twice_shares_one_slot() {
        let wav = TempWav::new("shared", 22_050);
        let mut app = headless_app();
        app.handle(Event::ImportSample {
            track: 2,
            path: wav.path(),
            start_beats: 0.0,
        });
        app.handle(Event::ImportSample {
            track: 3,
            path: wav.path(),
            start_beats: 0.0,
        });
        assert_eq!(app.samples.len(), 1);
        assert_eq!(app.mirror(2)[0].slot, app.mirror(3)[0].slot);
    }

    #[test]
    fn an_unreadable_file_reports_instead_of_loading() {
        let mut app = headless_app();
        app.handle(Event::ImportSample {
            track: 2,
            path: "/nonexistent/cassette.wav".into(),
            start_beats: 0.0,
        });
        assert!(app.song.tracks[2].blocks.is_empty());
        assert!(app.status.contains("cannot read"), "status: {}", app.status);
    }

    #[test]
    fn a_full_slot_table_refuses_the_load() {
        let wav = TempWav::new("busy", 2_205);
        let mut app = headless_app();
        for slot in 0..MAX_SAMPLE_SLOTS as u8 {
            app.samples.insert(format!("/tmp/taken-{slot}.wav"), slot);
        }
        app.handle(Event::ImportSample {
            track: 2,
            path: wav.path(),
            start_beats: 0.0,
        });
        assert!(app.song.tracks[2].blocks.is_empty());
        assert!(app.status.contains("busy"), "status: {}", app.status);
    }

    #[test]
    fn a_full_slot_table_refuses_before_touching_the_file() {
        let mut app = headless_app();
        for slot in 0..MAX_SAMPLE_SLOTS as u8 {
            app.samples.insert(format!("/tmp/taken-{slot}.wav"), slot);
        }
        app.handle(Event::ImportSample {
            track: 2,
            path: "/nonexistent/cassette.wav".into(),
            start_beats: 0.0,
        });
        assert!(app.status.contains("busy"), "status: {}", app.status);
    }

    #[test]
    fn a_truncated_or_cut_short_decode_is_reported() {
        let clean = Sample {
            frames: vec![0.0],
            sample_rate: 44_100,
            source_channels: 1,
            source_frames: 1,
            truncated: false,
            error: None,
        };
        assert_eq!(decode_note(&clean), None);
        let cut = Sample {
            truncated: true,
            error: Some("boom".into()),
            ..clean
        };
        let note = decode_note(&cut).expect("a note");
        assert!(
            note.contains("truncated") && note.contains("boom"),
            "{note}"
        );
    }

    #[test]
    fn a_sample_load_lands_in_the_lowest_free_slot() {
        let wav = TempWav::new("lowest", 2_205);
        let mut app = headless_app();
        app.samples.insert("/tmp/taken.wav".into(), 0);
        app.handle(Event::ImportSample {
            track: 2,
            path: wav.path(),
            start_beats: 0.0,
        });
        assert_eq!(app.samples.get(&wav.path()), Some(&1));
    }

    #[test]
    fn peaks_reduce_a_waveform_to_unit_range() {
        let frames: Vec<f32> = (0..1_024).map(|frame| frame as f32 / 1_023.0).collect();
        let peaks = peaks(&frames, WAVEFORM_BUCKETS);
        assert_eq!(peaks.len(), WAVEFORM_BUCKETS);
        assert!(peaks.iter().all(|peak| (0.0..=1.0).contains(peak)));
        assert!(peaks.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(peaks[WAVEFORM_BUCKETS - 1], 1.0);
    }

    #[test]
    fn peaks_of_silence_are_flat() {
        assert_eq!(peaks(&[0.0; 64], 4), vec![0.0; 4]);
        assert!(peaks(&[], 8).is_empty());
    }

    #[test]
    fn peaks_survive_more_buckets_than_frames() {
        let peaks = peaks(&[0.5, 0.25], 8);
        assert_eq!(peaks.len(), 8);
        assert_eq!(peaks[7], 0.25);
    }
}
