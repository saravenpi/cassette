//! The heart of the audio path: synth + transport + block scheduling.
//!
//! [`RenderCore`] is deliberately free of threads and I/O so it can be driven
//! from a real-time callback without allocating. The block mirror it keeps is
//! pushed in by the control thread as a `Vec`; the render path only reads it.

use crate::core::MAX_TRACKS;
use crate::core::sample::{NO_SLOT, SampleMsg};
use crate::core::synth::SynthEngine;
use crate::models::Song;
use crate::utils::time::Tempo;

/// Pitch a scheduled sample is played at. Sample players ignore it; it only has
/// to be a value the voice arrays never see from a keyboard.
pub const EVENT_PITCH: u8 = 255;

/// One block, flattened for the audio thread. The control side computes the
/// release-tail gain, so the render path needs no instrument lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MirroredBlock {
    /// Start on the timeline, in beats.
    pub start_beats: f32,
    /// End on the timeline, in beats.
    pub end_beats: f32,
    /// Sample slot holding the frames, or [`NO_SLOT`] for a MIDI block.
    pub slot: u8,
    /// Amplitude the voice starts at, which is the release tail for a block
    /// whose material runs past its end.
    pub gain: f32,
}

impl MirroredBlock {
    /// Whether the block covers `beats`.
    pub fn covers(&self, beats: f32) -> bool {
        beats >= self.start_beats && beats < self.end_beats
    }

    /// Whether the block's start falls in the half-open window `[from, to)`.
    ///
    /// Half-open so a block landing exactly on a buffer boundary starts in one
    /// buffer only, and `from` inclusive so the block at the very top of the
    /// song still fires on the first buffer.
    pub fn starts_in(&self, from: f32, to: f32) -> bool {
        self.start_beats >= from && self.start_beats < to
    }

    /// Whether the block's end falls in the half-open window `[from, to)`.
    pub fn ends_in(&self, from: f32, to: f32) -> bool {
        self.end_beats >= from && self.end_beats < to
    }
}

/// Every message the control side can send into the audio thread.
///
/// `SetBlocks` carries a `Vec`, so it allocates on the sending side — the app
/// thread — and the render path only ever drops or clones from one it already
/// owns. Every other variant is `Copy`.
#[derive(Clone, Debug, PartialEq)]
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
    /// Replace one track's block mirror outright.
    SetBlocks {
        track: u8,
        blocks: Vec<MirroredBlock>,
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
        instrument: crate::models::Instrument,
    },
    SetTrackMix {
        track: u8,
        volume: f32,
        pan: f32,
        muted: bool,
        solo: bool,
    },
    SetSampleRate {
        slot: u8,
        rate: f32,
    },
    Seek {
        beats: f32,
    },
    Play,
    Stop,
}

/// The frame clock. It advances a position and lets the UI read back where the
/// song is; [`RenderCore`] uses the same position to start blocks.
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

    pub fn frames_per_beat(&self) -> u64 {
        self.frames_per_beat
    }

    pub fn seek_beats(&mut self, beats: f32) {
        let safe = beats.max(0.0);
        self.position_frames = (safe as f64 * self.frames_per_beat as f64).round() as u64;
    }

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
    blocks: [Vec<MirroredBlock>; MAX_TRACKS],
    /// Peaks from the most recent [`RenderCore::process`] call.
    peaks: [f32; MAX_TRACKS],
}

impl RenderCore {
    /// Build a render core that mirrors `song`'s tracks and mixer.
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
            blocks: std::array::from_fn(|_| Vec::new()),
            peaks: [0.0; MAX_TRACKS],
        }
    }

    /// Sample rate this core was built for.
    pub fn sample_rate(&self) -> u32 {
        self.synth.sample_rate()
    }

    /// Replace one track's block mirror. Allocates on the calling side only.
    pub fn set_blocks(&mut self, track: usize, blocks: Vec<MirroredBlock>) {
        if let Some(slot) = self.blocks.get_mut(track) {
            *slot = blocks;
        }
    }

    /// Apply a command. Allocation-free once `SetBlocks`'s `Vec` is in hand.
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
            RtCommand::SetBlocks { track, blocks } => self.set_blocks(track as usize, blocks),
            RtCommand::SetTrackCount { count } => {
                self.synth.set_track_count(count as usize);
                for slot in &mut self.blocks {
                    slot.clear();
                }
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
            RtCommand::SetSampleRate { slot, rate } => {
                self.synth.set_sample_rate(slot, rate);
            }
            RtCommand::Seek { beats } => {
                self.transport.seek_beats(beats);
                self.synth.all_notes_off();
            }
            RtCommand::Play => self.transport.play(),
            RtCommand::Stop => {
                self.transport.stop();
                self.synth.all_notes_off();
            }
        }
    }

    /// Hand the render core the control thread's end of the sample ring.
    pub fn set_sample_ring(&mut self, consumer: rtrb::Consumer<SampleMsg>) {
        self.synth.set_sample_ring(consumer);
    }

    /// Move everything waiting on the sample ring into the pool. Called once per
    /// callback, before commands, so a load is audible the moment it arrives.
    pub fn drain_samples(&mut self) {
        self.synth.drain_samples();
    }

    /// Push one message into the sample pool directly, for tests.
    pub fn push_sample(&mut self, message: SampleMsg) {
        self.synth.push_sample(message);
    }

    /// Render interleaved stereo `out`. Returns the per-track peaks observed.
    ///
    /// Blocks are started *before* the buffer, so a sample whose block begins
    /// inside this buffer sounds in it from the first frame instead of losing
    /// its opening. The cost is that a block can start up to one buffer early,
    /// which is the resolution the transport schedules at.
    pub fn process(&mut self, out: &mut [f32]) -> [f32; MAX_TRACKS] {
        let mut peaks = [0.0f32; MAX_TRACKS];
        let frames = out.len() / 2;
        if frames > 0 && self.transport.is_playing() {
            let frames_per_beat = self.transport.frames_per_beat();
            let from_beats = self.transport.position_beats();
            let to_beats =
                (self.transport.position_frames() + frames as u64) as f32 / frames_per_beat as f32;
            self.start_blocks(from_beats, to_beats);
        }
        for frame in 0..frames {
            self.transport.tick_frame();
            let (left, right) = self.synth.process_frame(&mut peaks);
            out[frame * 2] = left;
            out[frame * 2 + 1] = right;
        }
        self.peaks = peaks;
        peaks
    }

    /// Start every block whose start falls in the window this buffer covers,
    /// and release every block the playhead has passed the end of.
    ///
    /// A block already behind the playhead when its buffer arrives is entered at
    /// the offset the timeline implies, so its content stays in step rather than
    /// restarting from the top.
    fn start_blocks(&mut self, from_beats: f32, to_beats: f32) {
        let frames_per_beat = self.transport.frames_per_beat() as f32;
        for track in 0..MAX_TRACKS {
            for (index, block) in self.blocks[track].iter().enumerate() {
                if block.slot == NO_SLOT {
                    continue;
                }
                if block.ends_in(from_beats, to_beats) {
                    self.synth.release_sample(track, index as u8);
                    continue;
                }
                let should_start = block.starts_in(from_beats, to_beats)
                    || (block.covers(from_beats) && !self.synth.is_sample_playing(track));
                if !should_start {
                    continue;
                }
                let elapsed = (from_beats - block.start_beats).max(0.0) * frames_per_beat;
                let offset = elapsed as u32;
                self.synth.play_sample(
                    track,
                    EVENT_PITCH,
                    block.slot,
                    offset,
                    block.gain,
                    index as u8,
                );
            }
        }
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
        self.blocks.get(track).map_or(0, Vec::len)
    }

    /// The mirrored block at `index`, if it exists.
    pub fn block(&self, track: usize, index: usize) -> Option<&MirroredBlock> {
        self.blocks.get(track)?.get(index)
    }

    /// How many samples the pool currently holds.
    pub fn sample_slots_ready(&self) -> usize {
        self.synth.sample_slots_ready()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DEFAULT_SAMPLE_RATE;
    use crate::core::sample::{MAX_SAMPLE_SLOTS, SampleMsg};

    fn render(core: &mut RenderCore, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        core.process(&mut out);
        out
    }

    fn blocks(spans: &[(f32, f32)]) -> Vec<MirroredBlock> {
        spans
            .iter()
            .map(|(start, end)| MirroredBlock {
                start_beats: *start,
                end_beats: *end,
                slot: 0,
                gain: 1.0,
            })
            .collect()
    }

    /// Fill slot 0 with `frames` frames of a constant value.
    fn load_slot(core: &mut RenderCore, frames: usize, value: f32) {
        core.push_sample(SampleMsg::Begin {
            slot: 0,
            frames: frames as u32,
            rate: 44_100.0,
        });
        for _ in 0..frames {
            core.push_sample(SampleMsg::Frame(value));
        }
        core.push_sample(SampleMsg::End { slot: 0 });
    }

    #[test]
    fn a_song_with_no_blocks_is_silent_but_still_moves() {
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
        let out = render(&mut core, 512);
        assert!(out.iter().any(|sample| sample.abs() > 0.001));
    }

    #[test]
    fn blocks_replace_the_whole_mirror() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 4.0), (8.0, 12.0)]),
        });
        assert_eq!(core.block_count(0), 2);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(16.0, 20.0)]),
        });
        assert_eq!(
            core.block_count(0),
            1,
            "a mirror is replaced, not appended to"
        );
        assert_eq!(core.block(0, 0).map(|b| b.start_beats), Some(16.0));
    }

    #[test]
    fn an_out_of_range_track_is_ignored() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 200,
            blocks: blocks(&[(0.0, 4.0)]),
        });
        assert_eq!(core.block_count(0), 0);
    }

    #[test]
    fn rebuilding_the_track_count_clears_the_block_mirror() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 4.0)]),
        });
        core.apply(RtCommand::SetTrackCount {
            count: song.tracks.len() as u8,
        });
        assert_eq!(core.block_count(0), 0);
    }

    #[test]
    fn a_block_started_by_the_playhead_plays_its_sample() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 44_100, 0.5);
        assert_eq!(core.sample_slots_ready(), 1);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 4.0)]),
        });
        core.apply(RtCommand::Play);
        let mut peak = 0.0f32;
        for _ in 0..3 {
            for sample in render(&mut core, 8_192) {
                peak = peak.max(sample.abs());
            }
        }
        assert!(peak > 0.01, "expected the sample to sound, got {peak}");
    }

    #[test]
    fn a_block_at_the_top_of_the_song_is_not_left_silent() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 44_100, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 8.0)]),
        });
        core.apply(RtCommand::Play);
        render(&mut core, 512);
        let out = render(&mut core, 512);
        let peak = out.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
        assert!(
            peak > 0.01,
            "the first block on a lane must sound, got {peak}"
        );
    }

    #[test]
    fn a_sample_longer_than_its_block_is_cut_at_the_block_end() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 44_100, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 1.0)]),
        });
        core.apply(RtCommand::Play);
        let mut loud = 0;
        for _ in 0..43 {
            if render(&mut core, 512).iter().any(|s| s.abs() > 1e-6) {
                loud += 1;
            }
        }
        assert_eq!(loud, 43, "the sample should play inside its one-beat block");
        render(&mut core, 512);
        for buffer in 0..15 {
            assert!(
                render(&mut core, 512).iter().all(|s| s.abs() < 1e-6),
                "buffer {buffer} after the block end still sounds"
            );
        }
        assert!(core.is_playing(), "the transport keeps running");
    }

    #[test]
    fn a_block_fires_once_per_pass_not_once_per_buffer() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 200, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 64.0)]),
        });
        core.apply(RtCommand::Play);
        let mut loud_buffers = 0;
        for _ in 0..16 {
            if render(&mut core, 512).iter().any(|s| s.abs() > 1e-6) {
                loud_buffers += 1;
            }
        }
        assert_eq!(
            loud_buffers, 1,
            "a sample shorter than a buffer must not restart on the next one"
        );
    }

    #[test]
    fn a_block_starting_on_a_buffer_boundary_fires_exactly_once() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 44_100, 64.0);
        let frames_per_beat = DEFAULT_SAMPLE_RATE as f32 / 2.0;
        let boundary_beats = 512.0 / frames_per_beat;
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(boundary_beats, boundary_beats + 64.0)]),
        });
        core.apply(RtCommand::Play);
        let mut voice_starts = 0;
        let mut last_was_silent = true;
        for _ in 0..16 {
            let out = render(&mut core, 512);
            let loud = out.iter().any(|s| s.abs() > 1e-6);
            if loud && last_was_silent {
                voice_starts += 1;
            }
            last_was_silent = !loud;
        }
        assert_eq!(voice_starts, 1, "the block started more than once");
    }

    #[test]
    fn a_block_that_never_starts_stays_quiet() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 44_100, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(64.0, 68.0)]),
        });
        core.apply(RtCommand::Play);
        let out = render(&mut core, 8_192);
        assert!(
            out.iter().all(|sample| sample.abs() < 1e-9),
            "a block later in the song must not fire early"
        );
    }

    #[test]
    fn a_block_holding_no_sample_makes_no_voice() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 4.0)]),
        });
        core.apply(RtCommand::Play);
        let out = render(&mut core, 8_192);
        assert!(out.iter().all(|sample| sample.abs() < 1e-9));
    }

    #[test]
    fn a_sample_is_entered_at_the_timeline_offset() {
        // A block that starts two beats in, with the transport already there.
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 44_100, 1.0);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 4.0)]),
        });
        core.apply(RtCommand::Play);
        render(&mut core, 44_100);
        assert!(
            core.position_beats() >= 1.9,
            "one second is two beats at 120"
        );
    }

    #[test]
    fn every_sample_slot_fits_the_pool() {
        assert!(
            MAX_SAMPLE_SLOTS < NO_SLOT as usize,
            "NO_SLOT must be out of band"
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
        assert!((transport.position_beats() - 1.0).abs() < 1e-2);
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
    }

    #[test]
    fn a_block_covers_its_span_and_not_its_end() {
        let block = MirroredBlock {
            start_beats: 2.0,
            end_beats: 6.0,
            slot: 0,
            gain: 1.0,
        };
        assert!(block.covers(2.0));
        assert!(!block.covers(6.0));
    }
}
