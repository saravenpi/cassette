//! The heart of the audio path: synth + transport + command application.
//!
//! [`RenderCore`] is deliberately free of threads and I/O so it can be driven
//! from a real-time callback without allocating.

use crate::core::synth::SynthEngine;
use crate::core::{MAX_BLOCKS, MAX_TRACKS};
use crate::models::{Instrument, Song};
use crate::utils::time::Tempo;

/// A block label that can cross into the audio thread without allocating.
///
/// Names are truncated to [`BlockName::CAPACITY`] bytes on a character
/// boundary, so [`BlockName::as_str`] always yields valid UTF-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockName {
    bytes: [u8; Self::CAPACITY],
    len: u8,
}

impl BlockName {
    /// Bytes a name may occupy.
    pub const CAPACITY: usize = 24;
    /// The empty name.
    pub const EMPTY: BlockName = BlockName {
        bytes: [0; Self::CAPACITY],
        len: 0,
    };

    /// Copy as much of `name` as fits.
    pub fn new(name: &str) -> Self {
        let mut bytes = [0u8; Self::CAPACITY];
        let mut len = 0usize;
        for character in name.chars() {
            let size = character.len_utf8();
            if len + size > Self::CAPACITY {
                break;
            }
            let mut encoded = [0u8; 4];
            bytes[len..len + size].copy_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            len += size;
        }
        Self {
            bytes,
            len: len as u8,
        }
    }

    /// The stored name.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }
}

/// Every message the control side can send into the audio thread.
///
/// All variants are `Copy`, so pushing one into the `rtrb` ring buffer never
/// allocates on either side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RtCommand {
    NoteOn {
        track: u8,
        pitch: u8,
        velocity: u8,
    },
    NoteOff {
        track: u8,
        pitch: u8,
    },
    AllNotesOff,
    AddBlock {
        track: u8,
        name: BlockName,
    },
    SetTrackCount {
        count: u8,
    },
    SetBpm {
        bpm: f32,
    },
    SetLooping {
        looping: bool,
    },
    SetTrackInstrument {
        track: u8,
        instrument: Instrument,
    },
    SetTrackMix {
        track: u8,
        volume: f32,
        pan: f32,
        muted: bool,
        solo: bool,
    },
    Play,
    Stop,
}

/// The frame clock. It schedules nothing yet: it advances a position and lets
/// the UI read back where the song is.
#[derive(Clone, Copy, Debug)]
pub struct Transport {
    bpm: f32,
    sample_rate: u32,
    frames_per_beat: u64,
    position_frames: u64,
    playing: bool,
    looping: bool,
}

impl Transport {
    /// A stopped transport at `bpm`.
    pub fn new(bpm: f32, sample_rate: u32) -> Self {
        let bpm = bpm.clamp(20.0, 400.0);
        Self {
            bpm,
            sample_rate,
            frames_per_beat: Tempo::new(bpm).frames_per_beat(sample_rate),
            position_frames: 0,
            playing: false,
            looping: true,
        }
    }

    /// Current speed in beats per minute.
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Change tempo, keeping the musical position so timing never drifts.
    pub fn set_bpm(&mut self, bpm: f32) {
        let beats = self.position_beats();
        self.bpm = bpm.clamp(20.0, 400.0);
        self.frames_per_beat = Tempo::new(self.bpm).frames_per_beat(self.sample_rate);
        self.position_frames = (beats as f64 * self.frames_per_beat as f64).round() as u64;
    }

    /// Frames in one beat at the current tempo.
    pub fn frames_per_beat(&self) -> u64 {
        self.frames_per_beat
    }

    /// Start (or resume) playback from the current position.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stop and rewind.
    pub fn stop(&mut self) {
        self.playing = false;
        self.position_frames = 0;
    }

    /// Enable or disable looping.
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    /// Whether the transport is running.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Whether looping is enabled.
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// Frames since the transport last started from the top.
    pub fn position_frames(&self) -> u64 {
        self.position_frames
    }

    /// Position in beats.
    pub fn position_beats(&self) -> f32 {
        self.position_frames as f32 / self.frames_per_beat as f32
    }

    /// Advance exactly one frame.
    pub fn tick_frame(&mut self) {
        if self.playing {
            self.position_frames += 1;
        }
    }
}

/// Synthesis + transport state owned by exactly one thread.
pub struct RenderCore {
    synth: SynthEngine,
    transport: Transport,
    blocks: [[BlockName; MAX_BLOCKS]; MAX_TRACKS],
    block_counts: [u8; MAX_TRACKS],
    /// Peaks from the most recent [`RenderCore::process`] call.
    peaks: [f32; MAX_TRACKS],
}

impl RenderCore {
    /// Build a render core that mirrors `song`.
    pub fn new(song: &Song, sample_rate: u32) -> Self {
        let mut synth = SynthEngine::new(sample_rate);
        synth.set_track_count(song.tracks.len());
        for (i, track) in song.tracks.iter().enumerate() {
            synth.configure_track(
                i,
                track.instrument,
                track.volume,
                track.pan,
                track.muted,
                track.solo,
            );
        }
        Self {
            synth,
            transport: Transport::new(song.bpm(), sample_rate),
            blocks: [[BlockName::EMPTY; MAX_BLOCKS]; MAX_TRACKS],
            block_counts: [0; MAX_TRACKS],
            peaks: [0.0; MAX_TRACKS],
        }
    }

    /// Sample rate this core was built for.
    pub fn sample_rate(&self) -> u32 {
        self.synth.sample_rate()
    }

    /// Apply a command. Allocation-free.
    pub fn apply(&mut self, command: RtCommand) {
        match command {
            RtCommand::NoteOn {
                track,
                pitch,
                velocity,
            } => self
                .synth
                .note_on(track as usize, pitch, velocity, u32::MAX),
            RtCommand::NoteOff { track, pitch } => self.synth.note_off(track as usize, pitch),
            RtCommand::AllNotesOff => self.synth.all_notes_off(),
            RtCommand::AddBlock { track, name } => self.add_block(track as usize, name),
            RtCommand::SetTrackCount { count } => {
                let count = (count as usize).min(MAX_TRACKS);
                self.synth.set_track_count(count);
                self.blocks = [[BlockName::EMPTY; MAX_BLOCKS]; MAX_TRACKS];
                self.block_counts = [0; MAX_TRACKS];
            }
            RtCommand::SetBpm { bpm } => self.transport.set_bpm(bpm),
            RtCommand::SetLooping { looping } => self.transport.set_looping(looping),
            RtCommand::SetTrackInstrument { track, instrument } => {
                self.synth.set_track_instrument(track as usize, instrument)
            }
            RtCommand::SetTrackMix {
                track,
                volume,
                pan,
                muted,
                solo,
            } => self
                .synth
                .set_track_mix(track as usize, volume, pan, muted, solo),
            RtCommand::Play => self.transport.play(),
            RtCommand::Stop => {
                self.transport.stop();
                self.synth.all_notes_off();
            }
        }
    }

    fn add_block(&mut self, track: usize, name: BlockName) {
        let Some(slots) = self.blocks.get_mut(track) else {
            return;
        };
        let Some(count) = self.block_counts.get_mut(track) else {
            return;
        };
        let index = *count as usize;
        if index < MAX_BLOCKS {
            slots[index] = name;
            *count += 1;
        }
    }

    /// Render interleaved stereo `out`. Returns the per-track peaks observed.
    pub fn process(&mut self, out: &mut [f32]) -> [f32; MAX_TRACKS] {
        let mut peaks = [0.0f32; MAX_TRACKS];
        let frames = out.len() / 2;
        for frame in 0..frames {
            self.transport.tick_frame();
            let (left, right) = self.synth.process_frame(&mut peaks);
            out[frame * 2] = left;
            out[frame * 2 + 1] = right;
        }
        self.peaks = peaks;
        peaks
    }

    /// Peaks recorded by the last [`RenderCore::process`] call.
    pub fn peaks(&self) -> [f32; MAX_TRACKS] {
        self.peaks
    }

    /// Transport position in frames.
    pub fn position_frames(&self) -> u64 {
        self.transport.position_frames()
    }

    /// Position in beats.
    pub fn position_beats(&self) -> f32 {
        self.transport.position_beats()
    }

    /// Whether the transport is running.
    pub fn is_playing(&self) -> bool {
        self.transport.is_playing()
    }

    /// Whether looping is enabled.
    pub fn is_looping(&self) -> bool {
        self.transport.is_looping()
    }

    /// How many blocks the audio side holds for `track`.
    pub fn block_count(&self, track: usize) -> usize {
        self.block_counts.get(track).copied().unwrap_or(0) as usize
    }

    /// Name of a mirrored block, or `""` when the track holds no block at `index`.
    pub fn block_name(&self, track: usize, index: usize) -> &str {
        if index >= self.block_count(track) {
            return "";
        }
        self.blocks
            .get(track)
            .and_then(|slots| slots.get(index))
            .map(BlockName::as_str)
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DEFAULT_SAMPLE_RATE;

    fn render(core: &mut RenderCore, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        core.process(&mut out);
        out
    }

    #[test]
    fn block_names_are_truncated_on_a_character_boundary() {
        let name = BlockName::new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(name.as_str().len(), BlockName::CAPACITY);
        let accented = BlockName::new("ééééééééééééééééééééééééé");
        assert!(accented.as_str().len() <= BlockName::CAPACITY);
        assert!(accented.as_str().chars().all(|c| c == 'é'));
    }

    #[test]
    fn blocks_round_trip_through_the_render_core() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::AddBlock {
            track: 0,
            name: BlockName::new("Verse"),
        });
        core.apply(RtCommand::AddBlock {
            track: 0,
            name: BlockName::new("Chorus"),
        });
        assert_eq!(core.block_count(0), 2);
        assert_eq!(core.block_name(0, 1), "Chorus");
        assert_eq!(core.block_name(0, 9), "", "past the count is never a name");
    }

    #[test]
    fn rebuilding_the_track_count_clears_the_block_mirror() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        for name in ["Verse", "Chorus"] {
            core.apply(RtCommand::AddBlock {
                track: 0,
                name: BlockName::new(name),
            });
        }
        assert_eq!(core.block_count(0), 2);

        core.apply(RtCommand::SetTrackCount {
            count: song.tracks.len() as u8,
        });
        assert_eq!(core.block_count(0), 0);
        assert_eq!(
            core.block_name(0, 0),
            "",
            "a resync must not resurrect stale names"
        );
    }

    #[test]
    fn transport_advances_one_beat_per_frames_per_beat() {
        let mut transport = Transport::new(120.0, DEFAULT_SAMPLE_RATE);
        let frames_per_beat = transport.frames_per_beat();
        transport.play();
        for _ in 0..frames_per_beat {
            transport.tick_frame();
        }
        assert!((transport.position_beats() - 1.0).abs() < 1e-3);
        assert!(transport.is_playing());
    }

    #[test]
    fn stop_rewinds_to_zero() {
        let mut transport = Transport::new(120.0, DEFAULT_SAMPLE_RATE);
        transport.play();
        for _ in 0..1_000 {
            transport.tick_frame();
        }
        transport.stop();
        assert_eq!(transport.position_frames(), 0);
        assert!(!transport.is_playing());
    }

    #[test]
    fn a_song_with_no_notes_is_silent() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::Play);
        let out = render(&mut core, 512);
        assert!(out.iter().all(|sample| *sample == 0.0));
        assert!(core.position_frames() > 0, "the transport still advances");
    }

    #[test]
    fn a_played_note_makes_sound() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::NoteOn {
            track: 0,
            pitch: 60,
            velocity: 100,
        });
        let mut peak = 0.0f32;
        for _ in 0..20 {
            for sample in render(&mut core, 512) {
                peak = peak.max(sample.abs());
            }
        }
        assert!(peak > 0.01, "expected audible output, got {peak}");
    }
}
