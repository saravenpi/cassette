//! The heart of the audio path: synth + transport + block scheduling.
//!
//! [`RenderCore`] is deliberately free of threads and I/O so it can be driven
//! from a real-time callback without allocating. The block mirror it keeps is
//! a fixed array filled by copying from a slice the control thread sends; the
//! render path never allocates or frees it. The note mirror works the same way.

use crate::core::sample::{NO_SLOT, SampleMsg};
use crate::core::synth::{SampleCue, SynthEngine};
use crate::core::{MAX_BLOCKS, MAX_NOTES, MAX_TRACKS};
use crate::models::{EffectChain, LoopRegion, Song};
use crate::utils::time::Tempo;
use std::sync::Arc;

/// Pitch a scheduled sample is played at. Sample players ignore it; it only has
/// to be a value the voice arrays never see from a keyboard.
pub const EVENT_PITCH: u8 = 255;

/// Beats in one bar; the metronome accents the first of each.
const BEATS_PER_BAR: u64 = 4;
/// Length of one metronome click, in seconds.
const CLICK_SECONDS: f32 = 0.015;
/// Time constant of the click's exponential decay, in seconds.
const CLICK_DECAY_SECONDS: f32 = 0.003;
/// Pitch and level of the bar's downbeat click.
const ACCENT_CLICK: (f32, f32) = (1_760.0, 0.5);
/// Pitch and level of every other beat's click.
const BEAT_CLICK: (f32, f32) = (1_046.5, 0.3);

/// The metronome's voice: one decaying sine burst, held in fixed state.
#[derive(Clone, Copy, Debug)]
struct Click {
    phase: f32,
    step: f32,
    amplitude: f32,
    decay: f32,
    frames_left: u32,
}

impl Click {
    const SILENT: Self = Self {
        phase: 0.0,
        step: 0.0,
        amplitude: 0.0,
        decay: 0.0,
        frames_left: 0,
    };

    /// Start a click, higher and louder on a bar's downbeat.
    fn trigger(&mut self, accent: bool, sample_rate: u32) {
        let (frequency, amplitude) = if accent { ACCENT_CLICK } else { BEAT_CLICK };
        let rate = sample_rate as f32;
        *self = Self {
            phase: 0.0,
            step: std::f32::consts::TAU * frequency / rate,
            amplitude,
            decay: (-1.0 / (CLICK_DECAY_SECONDS * rate)).exp(),
            frames_left: (CLICK_SECONDS * rate) as u32,
        };
    }

    /// The click's next sample, zero once it has run out.
    fn next_sample(&mut self) -> f32 {
        if self.frames_left == 0 {
            return 0.0;
        }
        self.frames_left -= 1;
        self.phase = (self.phase + self.step) % std::f32::consts::TAU;
        let sample = self.amplitude * self.phase.sin();
        self.amplitude *= self.decay;
        sample
    }
}

/// Where `frame` sits on a grid of beats `frames_per_beat` apart, each beat on
/// its rounded frame: the beat that starts on `frame`, if one does, and the
/// frames until the next beat after it.
fn beat_grid(frame: u64, frames_per_beat: f64) -> (Option<u64>, u64) {
    let below = (frame as f64 / frames_per_beat).floor() as u64;
    let mut on = None;
    let mut until = u64::MAX;
    for beat in [below, below + 1, below + 2] {
        let start = (beat as f64 * frames_per_beat).round() as u64;
        if start == frame {
            on = Some(beat);
        } else if start > frame {
            until = until.min(start - frame);
        }
    }
    (on, until)
}

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
    /// Seconds into the material where the block starts reading.
    pub offset_seconds: f32,
    /// Speed the material plays at, relative to its own.
    pub rate: f32,
}

impl MirroredBlock {
    /// Whether the block covers `beats`.
    pub fn covers(&self, beats: f64) -> bool {
        beats >= f64::from(self.start_beats) && beats < f64::from(self.end_beats)
    }

    /// Whether the block's start falls in the half-open window `[from, to)`.
    ///
    /// Half-open so a block landing exactly on a buffer boundary starts in one
    /// buffer only, and `from` inclusive so the block at the very top of the
    /// song still fires on the first buffer.
    pub fn starts_in(&self, from: f64, to: f64) -> bool {
        let start = f64::from(self.start_beats);
        start >= from && start < to
    }

    /// Whether the block's end falls in the half-open window `[from, to)`.
    pub fn ends_in(&self, from: f64, to: f64) -> bool {
        let end = f64::from(self.end_beats);
        end >= from && end < to
    }
}

const EMPTY_BLOCK: MirroredBlock = MirroredBlock {
    start_beats: 0.0,
    end_beats: 0.0,
    slot: NO_SLOT,
    gain: 0.0,
    offset_seconds: 0.0,
    rate: 1.0,
};

/// One note, flattened for the audio thread: absolute beats on the timeline,
/// already clipped to its block's end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MirroredNote {
    /// Start on the timeline, in beats.
    pub start_beats: f32,
    /// End on the timeline, in beats.
    pub end_beats: f32,
    pub pitch: u8,
    pub velocity: u8,
}

impl MirroredNote {
    /// Whether the note's start falls in the half-open window `[from, to)`,
    /// with the same edges as [`MirroredBlock::starts_in`].
    pub fn starts_in(&self, from: f64, to: f64) -> bool {
        let start = f64::from(self.start_beats);
        start >= from && start < to
    }

    /// Frames the note is held for at `frames_per_beat`, at least one and never
    /// the synth's held marker.
    pub fn gate_frames(&self, frames_per_beat: f64) -> u32 {
        let length = f64::from(self.end_beats - self.start_beats) * frames_per_beat;
        length.round().clamp(1.0, f64::from(u32::MAX - 1)) as u32
    }
}

const EMPTY_NOTE: MirroredNote = MirroredNote {
    start_beats: 0.0,
    end_beats: 0.0,
    pitch: 0,
    velocity: 0,
};

/// Every message the control side can send into the audio thread.
///
/// `SetBlocks` and `SetNotes` carry an `Arc` slice, built on the sending side. The sender
/// must keep a clone until the audio thread has taken the command, so the audio
/// thread never drops the last reference. Every other variant is `Copy`.
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
        blocks: Arc<[MirroredBlock]>,
    },
    /// Replace one track's note mirror outright. Notes are sorted by start.
    SetNotes {
        track: u8,
        notes: Arc<[MirroredNote]>,
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
    /// Loop between two beats; an end at or before the start loops the song.
    SetLoopRegion {
        start_beats: f32,
        end_beats: f32,
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
    /// Replace one track's insert chain.
    SetEffects {
        track: u8,
        chain: crate::models::EffectChain,
    },
    /// Turn the metronome click on or off.
    SetMetronome {
        on: bool,
    },
    /// Click `beats` beats of count-in, then start the transport.
    PlayAfterCountIn {
        beats: u16,
    },
}

impl RtCommand {
    /// The command that sets `region`, or loops the whole song for `None`.
    pub fn loop_region(region: Option<LoopRegion>) -> Self {
        let (start_beats, end_beats) = region.map_or((0.0, 0.0), |r| (r.start_beats, r.end_beats));
        Self::SetLoopRegion {
            start_beats,
            end_beats,
        }
    }
}

/// The frame clock. It advances a position and lets the UI read back where the
/// song is; [`RenderCore`] uses the same position to start blocks.
#[derive(Clone, Copy, Debug)]
pub struct Transport {
    bpm: f32,
    sample_rate: u32,
    frames_per_beat: f64,
    position_frames: f64,
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
            position_frames: 0.0,
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
        let beats = self.position_beats_exact();
        self.bpm = bpm.clamp(20.0, 400.0);
        self.frames_per_beat = Tempo::new(self.bpm).frames_per_beat(self.sample_rate);
        self.position_frames = beats * self.frames_per_beat;
    }

    /// Frames in one beat at the current tempo, fractional so tempo never rounds.
    pub fn frames_per_beat(&self) -> f64 {
        self.frames_per_beat
    }

    pub fn seek_beats(&mut self, beats: f32) {
        self.position_frames = f64::from(beats.max(0.0)) * self.frames_per_beat;
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stop and rewind.
    pub fn stop(&mut self) {
        self.playing = false;
        self.position_frames = 0.0;
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
        self.position_frames as u64
    }

    /// Position in beats.
    pub fn position_beats(&self) -> f32 {
        self.position_beats_exact() as f32
    }

    /// Position in beats at full `f64` precision.
    pub fn position_beats_exact(&self) -> f64 {
        self.frames_to_beats(self.position_frames)
    }

    /// Convert a frame count to beats in `f64`, so long songs keep frame accuracy.
    pub fn frames_to_beats(&self, frames: f64) -> f64 {
        frames / self.frames_per_beat
    }

    /// Position in beats after `frames` more frames.
    pub fn beats_after(&self, frames: usize) -> f64 {
        self.frames_to_beats(self.position_frames + frames as f64)
    }

    /// Advance exactly one frame.
    pub fn tick_frame(&mut self) {
        if self.playing {
            self.position_frames += 1.0;
        }
    }
}

/// Synthesis + transport state owned by exactly one thread.
pub struct RenderCore {
    synth: SynthEngine,
    transport: Transport,
    blocks: [[MirroredBlock; MAX_BLOCKS]; MAX_TRACKS],
    block_lens: [usize; MAX_TRACKS],
    /// One note mirror per track, boxed because it is too large for a stack.
    notes: Box<[[MirroredNote; MAX_NOTES]]>,
    note_lens: [usize; MAX_TRACKS],
    /// End of the last mirrored block, in beats; the loop wraps here.
    song_end_beats: f32,
    /// Loop region in beats; the whole song while the end is not past the start.
    loop_start_beats: f32,
    loop_end_beats: f32,
    metronome: bool,
    click: Click,
    /// Frames of count-in pre-roll still to run before the transport starts.
    count_in_left: u64,
    /// Frames the current count-in pre-roll lasts in all.
    count_in_total: u64,
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
        for (i, track) in song.tracks.iter().enumerate() {
            synth.set_effects(i, &EffectChain::from_effects(&track.effects));
        }
        let mut core = Self {
            synth,
            transport: Transport::new(song.bpm(), sample_rate),
            blocks: [[EMPTY_BLOCK; MAX_BLOCKS]; MAX_TRACKS],
            block_lens: [0; MAX_TRACKS],
            notes: vec![[EMPTY_NOTE; MAX_NOTES]; MAX_TRACKS].into_boxed_slice(),
            note_lens: [0; MAX_TRACKS],
            song_end_beats: 0.0,
            loop_start_beats: song.loop_region().map_or(0.0, |r| r.start_beats),
            loop_end_beats: song.loop_region().map_or(0.0, |r| r.end_beats),
            metronome: false,
            click: Click::SILENT,
            count_in_left: 0,
            count_in_total: 0,
        };
        core.set_tempo(song.bpm());
        core
    }

    /// Set the tempo on the transport and on the synth's tempo-synced effects.
    fn set_tempo(&mut self, bpm: f32) {
        self.transport.set_bpm(bpm);
        self.synth.set_tempo(self.transport.bpm());
    }

    /// Sample rate this core was built for.
    pub fn sample_rate(&self) -> u32 {
        self.synth.sample_rate()
    }

    /// Replace one track's block mirror, keeping at most [`MAX_BLOCKS`] blocks.
    /// Copies into fixed storage, so it neither allocates nor frees.
    pub fn set_blocks(&mut self, track: usize, blocks: impl AsRef<[MirroredBlock]>) {
        let blocks = blocks.as_ref();
        let (Some(slot), Some(len)) = (self.blocks.get_mut(track), self.block_lens.get_mut(track))
        else {
            return;
        };
        let kept = blocks.len().min(MAX_BLOCKS);
        slot[..kept].copy_from_slice(&blocks[..kept]);
        *len = kept;
        self.refresh_song_end();
        self.follow_sample(track);
    }

    /// Point a playing sample voice at the block that now covers the playhead.
    ///
    /// A voice is tagged with its block's index, and a split, a duplicate or a
    /// move re-sorts the lane under it; without this the block's end would no
    /// longer match the tag and the sample would ring on. A voice with no
    /// block left under the playhead fades out.
    fn follow_sample(&mut self, track: usize) {
        if !self.synth.is_sample_playing(track) {
            return;
        }
        let beats = self.transport.position_beats_exact();
        let covering = self.blocks[track][..self.block_lens[track]]
            .iter()
            .position(|block| block.slot != NO_SLOT && block.covers(beats));
        match covering {
            Some(index) => self.synth.retag_sample(track, index as u8),
            None => self.synth.release_playing_sample(track),
        }
    }

    /// Replace one track's note mirror, keeping at most [`MAX_NOTES`] notes.
    /// Copies into fixed storage, so it neither allocates nor frees.
    pub fn set_notes(&mut self, track: usize, notes: impl AsRef<[MirroredNote]>) {
        let notes = notes.as_ref();
        let (Some(slot), Some(len)) = (self.notes.get_mut(track), self.note_lens.get_mut(track))
        else {
            return;
        };
        let kept = notes.len().min(MAX_NOTES);
        slot[..kept].copy_from_slice(&notes[..kept]);
        *len = kept;
    }

    fn refresh_song_end(&mut self) {
        let mut end = 0.0f32;
        for track in 0..MAX_TRACKS {
            for block in &self.blocks[track][..self.block_lens[track]] {
                end = end.max(block.end_beats);
            }
        }
        self.song_end_beats = end;
    }

    /// Apply a command. Allocation-free: a `SetBlocks` or `SetNotes` payload is
    /// copied, not kept.
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
            RtCommand::SetBlocks { track, blocks } => self.set_blocks(track as usize, &blocks),
            RtCommand::SetNotes { track, notes } => self.set_notes(track as usize, &notes),
            RtCommand::SetTrackCount { count } => {
                self.synth.set_track_count(count as usize);
                self.block_lens = [0; MAX_TRACKS];
                self.note_lens = [0; MAX_TRACKS];
                self.song_end_beats = 0.0;
            }
            RtCommand::SetBpm { bpm } => self.set_tempo(bpm),
            RtCommand::SetLooping { looping } => self.transport.set_looping(looping),
            RtCommand::SetLoopRegion {
                start_beats,
                end_beats,
            } => {
                self.loop_start_beats = start_beats;
                self.loop_end_beats = end_beats;
            }
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
                self.cancel_count_in();
                self.transport.seek_beats(beats);
                self.synth.all_notes_off();
            }
            RtCommand::Play => {
                self.cancel_count_in();
                self.transport.play();
            }
            RtCommand::Stop => {
                self.cancel_count_in();
                self.transport.stop();
                self.synth.all_notes_off();
            }
            RtCommand::SetEffects { track, chain } => {
                self.synth.set_effects(track as usize, &chain)
            }
            RtCommand::SetMetronome { on } => self.metronome = on,
            RtCommand::PlayAfterCountIn { beats } => self.count_in(beats),
        }
    }

    /// Start a pre-roll of `beats` beats that holds the playhead, or play at
    /// once for none or when the transport already runs.
    fn count_in(&mut self, beats: u16) {
        let frames = (f64::from(beats) * self.transport.frames_per_beat()).round() as u64;
        if frames == 0 || self.transport.is_playing() {
            self.cancel_count_in();
            self.transport.play();
            return;
        }
        self.count_in_left = frames;
        self.count_in_total = frames;
    }

    fn cancel_count_in(&mut self) {
        self.count_in_left = 0;
        self.count_in_total = 0;
    }

    /// Click the pre-roll beat that starts now, if any, and return how many of
    /// `wanted` frames run before the next beat or the end of the pre-roll.
    fn begin_count_in(&mut self, wanted: usize) -> usize {
        let elapsed = self.count_in_total - self.count_in_left;
        let (on, until) = beat_grid(elapsed, self.transport.frames_per_beat());
        if let Some(beat) = on {
            self.click
                .trigger(beat % BEATS_PER_BAR == 0, self.synth.sample_rate());
        }
        (wanted as u64).min(until).min(self.count_in_left) as usize
    }

    /// Run `frames` frames off the pre-roll, starting the transport the frame it
    /// ends.
    fn advance_count_in(&mut self, frames: usize) {
        self.count_in_left = self.count_in_left.saturating_sub(frames as u64);
        if self.count_in_left == 0 {
            self.count_in_total = 0;
            self.transport.play();
        }
    }

    /// Click the metronome beat the playhead sits on, if any, and return how
    /// many of `wanted` frames run before the next beat.
    fn begin_metronome_span(&mut self, wanted: usize) -> usize {
        let frame = self.transport.position_frames.round() as u64;
        let (on, until) = beat_grid(frame, self.transport.frames_per_beat());
        if let Some(beat) = on {
            self.click
                .trigger(beat % BEATS_PER_BAR == 0, self.synth.sample_rate());
        }
        (wanted as u64).min(until) as usize
    }

    /// Hand the render core the control thread's end of the sample ring.
    pub fn set_sample_ring(&mut self, consumer: rtrb::Consumer<SampleMsg>) {
        self.synth.set_sample_ring(consumer);
    }

    /// Apply everything waiting on the sample ring to the slots. Called once per
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
    /// The buffer is rendered in stretches cut at every loop end, block edge,
    /// note start and, while the metronome or a count-in clicks, every beat.
    /// Each stretch starts its blocks, notes and click before rendering, so they
    /// sound from the frame the timeline puts them on. The click is mixed into
    /// the master after the synth, so no track meter sees it.
    pub fn process(&mut self, out: &mut [f32]) -> [f32; MAX_TRACKS] {
        let mut peaks = [0.0f32; MAX_TRACKS];
        let frames = out.len() / 2;
        let mut done = 0;
        while done < frames {
            let mut span = frames - done;
            let counting_in = self.count_in_left > 0;
            if counting_in {
                span = self.begin_count_in(span);
            } else if self.transport.is_playing() {
                span = self.begin_span(span);
            }
            for frame in done..done + span {
                self.transport.tick_frame();
                let (left, right) = self.synth.process_frame(&mut peaks);
                let click = self.click.next_sample();
                out[frame * 2] = (left + click).clamp(-1.0, 1.0);
                out[frame * 2 + 1] = (right + click).clamp(-1.0, 1.0);
            }
            if counting_in {
                self.advance_count_in(span);
            }
            done += span;
        }
        peaks
    }

    /// The loop's start and end in beats: the region when one is set, else the
    /// top of the song to the end of its last block.
    fn loop_span(&self) -> (f32, f32) {
        if self.loop_end_beats > self.loop_start_beats {
            (self.loop_start_beats, self.loop_end_beats)
        } else {
            (0.0, self.song_end_beats)
        }
    }

    /// Frame the loop wraps at, when looping is on and the loop has a length.
    fn loop_end_frames(&self) -> Option<u64> {
        let end = self.loop_span().1;
        if !self.transport.is_looping() || end <= 0.0 {
            return None;
        }
        let frames = (f64::from(end) * self.transport.frames_per_beat()).round();
        (frames >= 1.0).then_some(frames as u64)
    }

    /// Wrap the playhead if it sits on the loop end, start the blocks and notes
    /// inside the next stretch, and return how many of `wanted` frames that
    /// stretch spans. A stretch never crosses the loop end, so the wrap lands on
    /// an exact frame, and a wrap releases every voice so no note rings across it.
    /// A stretch also stops where the next block or note begins or a sample block
    /// ends, so each starts on its own frame rather than at the top of a buffer.
    fn begin_span(&mut self, wanted: usize) -> usize {
        let mut span = wanted;
        if let Some(end) = self.loop_end_frames() {
            if self.transport.position_frames() >= end {
                self.transport.seek_beats(self.loop_span().0);
                self.synth.all_notes_off();
            }
            span = span.min(end.saturating_sub(self.transport.position_frames()).max(1) as usize);
        }
        let from_beats = self.transport.position_beats_exact();
        if let Some(next) = self.next_event_after(from_beats, self.transport.beats_after(span)) {
            let frames = ((next - from_beats) * self.transport.frames_per_beat()).ceil();
            span = span.min((frames as usize).max(1));
        }
        if self.metronome {
            span = self.begin_metronome_span(span);
        }
        let to_beats = self.transport.beats_after(span);
        self.start_blocks(from_beats, to_beats);
        self.start_notes(from_beats, to_beats);
        span
    }

    /// The earliest block start, sample block end or note start strictly inside
    /// `(from_beats, to_beats)`, where the stretch has to be cut.
    fn next_event_after(&self, from_beats: f64, to_beats: f64) -> Option<f64> {
        let inside = |beats: f32| {
            let beats = f64::from(beats);
            (beats > from_beats && beats < to_beats).then_some(beats)
        };
        let mut next: Option<f64> = None;
        let mut keep = |beats: Option<f64>| {
            if let Some(beats) = beats {
                next = Some(next.map_or(beats, |held| held.min(beats)));
            }
        };
        for track in 0..MAX_TRACKS {
            for block in &self.blocks[track][..self.block_lens[track]] {
                if block.slot != NO_SLOT {
                    keep(inside(block.start_beats));
                    keep(inside(block.end_beats));
                }
            }
            let notes = &self.notes[track][..self.note_lens[track]];
            let first = notes.partition_point(|note| f64::from(note.start_beats) <= from_beats);
            keep(notes.get(first).and_then(|note| inside(note.start_beats)));
        }
        next
    }

    /// Start every note whose start falls in the window this buffer covers,
    /// gated to its length. A note already under the playhead is not entered.
    /// The mirror is sorted by start, so the window is found by binary search.
    fn start_notes(&mut self, from_beats: f64, to_beats: f64) {
        let frames_per_beat = self.transport.frames_per_beat();
        for track in 0..MAX_TRACKS {
            let notes = &self.notes[track][..self.note_lens[track]];
            let first = notes.partition_point(|note| f64::from(note.start_beats) < from_beats);
            for note in notes[first..]
                .iter()
                .take_while(|note| note.starts_in(from_beats, to_beats))
            {
                self.synth.note_on(
                    track,
                    note.pitch,
                    note.velocity,
                    note.gate_frames(frames_per_beat),
                );
            }
        }
    }

    /// Start every block whose start falls in the window this buffer covers,
    /// and release every block the playhead has passed the end of.
    ///
    /// A block already behind the playhead when its buffer arrives is entered at
    /// the offset the timeline implies, so its content stays in step rather than
    /// restarting from the top.
    fn start_blocks(&mut self, from_beats: f64, to_beats: f64) {
        let frames_per_beat = self.transport.frames_per_beat();
        for track in 0..MAX_TRACKS {
            for (index, block) in self.blocks[track][..self.block_lens[track]]
                .iter()
                .enumerate()
            {
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
                let cue = SampleCue {
                    offset_seconds: block.offset_seconds,
                    elapsed_frames: (from_beats - f64::from(block.start_beats)).max(0.0)
                        * frames_per_beat,
                    rate: block.rate,
                };
                self.synth.play_sample(
                    track,
                    EVENT_PITCH,
                    block.slot,
                    cue,
                    block.gain,
                    index as u8,
                );
            }
        }
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

    /// Frames of count-in pre-roll still to run, zero when none is running.
    pub fn count_in_frames(&self) -> u64 {
        self.count_in_left
    }

    /// How many blocks the audio side holds for `track`.
    pub fn block_count(&self, track: usize) -> usize {
        self.block_lens.get(track).copied().unwrap_or(0)
    }

    /// How many notes the audio side holds for `track`.
    pub fn note_count(&self, track: usize) -> usize {
        self.note_lens.get(track).copied().unwrap_or(0)
    }

    /// The mirrored block at `index`, if it exists.
    pub fn block(&self, track: usize, index: usize) -> Option<&MirroredBlock> {
        if index >= *self.block_lens.get(track)? {
            return None;
        }
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

    fn blocks(spans: &[(f32, f32)]) -> Arc<[MirroredBlock]> {
        spans
            .iter()
            .map(|(start, end)| MirroredBlock {
                start_beats: *start,
                end_beats: *end,
                slot: 0,
                gain: 1.0,
                offset_seconds: 0.0,
                rate: 1.0,
            })
            .collect()
    }

    fn notes(spans: &[(f32, f32)]) -> Arc<[MirroredNote]> {
        spans
            .iter()
            .map(|(start, end)| MirroredNote {
                start_beats: *start,
                end_beats: *end,
                pitch: 60,
                velocity: 100,
            })
            .collect()
    }

    fn loud(out: &[f32]) -> bool {
        out.iter().any(|s| s.abs() > 1e-6)
    }

    /// Fill slot 0 with `frames` frames of a constant value.
    fn load_slot(core: &mut RenderCore, frames: usize, value: f32) {
        core.push_sample(SampleMsg::Load {
            slot: 0,
            frames: vec![value; frames].into(),
            rate: 44_100.0,
        });
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
        core.apply(RtCommand::SetNotes {
            track: 0,
            notes: notes(&[(0.0, 1.0)]),
        });
        core.apply(RtCommand::SetTrackCount {
            count: song.tracks.len() as u8,
        });
        assert_eq!(core.block_count(0), 0);
        assert_eq!(core.note_count(0), 0);
    }

    #[test]
    fn a_note_fires_in_the_buffer_holding_its_start_and_not_before() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::SetNotes {
            track: 0,
            notes: notes(&[(1.0, 2.0)]),
        });
        core.apply(RtCommand::Play);
        let first_loud = (0..64).find(|_| loud(&render(&mut core, 512)));
        assert_eq!(first_loud, Some(22_050 / 512), "beat one is frame 22050");
    }

    #[test]
    fn a_note_starts_on_its_own_frame_inside_a_buffer() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::SetNotes {
            track: 0,
            notes: notes(&[(1.0, 2.0)]),
        });
        core.apply(RtCommand::Play);
        let out = render(&mut core, 22_050 + 512);
        let first = out.iter().position(|s| s.abs() > 1e-6).map(|i| i / 2);
        assert!(!loud(&out[..22_050 * 2]), "nothing before beat one");
        assert!(
            first.is_some_and(|frame| frame <= 22_052),
            "the note opens on frame 22050, got {first:?}"
        );
    }

    #[test]
    fn a_note_is_released_after_its_gate() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::SetNotes {
            track: 0,
            notes: notes(&[(0.0, 0.5)]),
        });
        core.apply(RtCommand::Play);
        let held = render(&mut core, 11_000);
        assert!(
            loud(&held[held.len() - 64..]),
            "the note holds for its gate"
        );
        render(&mut core, 25 + 8_000);
        assert!(
            !loud(&render(&mut core, 512)),
            "the gate ran out and the release finished"
        );
    }

    #[test]
    fn a_loop_region_wraps_back_to_its_own_start() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLoopRegion {
            start_beats: 2.0,
            end_beats: 3.0,
        });
        core.apply(RtCommand::SetNotes {
            track: 0,
            notes: notes(&[(2.0, 2.5)]),
        });
        core.apply(RtCommand::Play);
        render(&mut core, 22_050 * 3);
        assert_eq!(
            core.position_beats(),
            3.0,
            "the playhead reached the loop end"
        );
        assert!(
            loud(&render(&mut core, 512)),
            "the wrap lands on the region start and its note plays again"
        );
        let beats = core.position_beats();
        assert!(
            (2.0..2.1).contains(&beats),
            "wrapped into the region: {beats}"
        );
        core.apply(RtCommand::SetLoopRegion {
            start_beats: 0.0,
            end_beats: 0.0,
        });
        assert_eq!(core.loop_span(), (0.0, 0.0), "cleared back to the song");
    }

    #[test]
    fn a_loop_wrap_cuts_a_note_that_crosses_the_loop_end() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: Arc::from(vec![MirroredBlock {
                start_beats: 0.0,
                end_beats: 1.0,
                slot: NO_SLOT,
                gain: 0.0,
                offset_seconds: 0.0,
                rate: 1.0,
            }]),
        });
        core.apply(RtCommand::SetNotes {
            track: 0,
            notes: notes(&[(0.9, 16.0)]),
        });
        core.apply(RtCommand::Play);
        let before_wrap = render(&mut core, 22_000);
        assert!(loud(&before_wrap[before_wrap.len() - 64..]));
        render(&mut core, 50 + 8_000);
        assert!(
            !loud(&render(&mut core, 512)),
            "the wrap released the note before it could ring into the next pass"
        );
    }

    #[test]
    fn a_seek_into_a_note_does_not_enter_it() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::SetNotes {
            track: 0,
            notes: notes(&[(0.0, 8.0)]),
        });
        core.apply(RtCommand::Seek { beats: 1.0 });
        core.apply(RtCommand::Play);
        assert!(!loud(&render(&mut core, 8_192)));
    }

    #[test]
    fn set_notes_keeps_at_most_max_notes() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        let spans: Vec<(f32, f32)> = (0..MAX_NOTES + 10)
            .map(|i| (i as f32, i as f32 + 0.5))
            .collect();
        core.set_notes(0, notes(&spans));
        assert_eq!(core.note_count(0), MAX_NOTES);
        core.set_notes(200, notes(&spans));
        assert_eq!(core.note_count(200), 0, "an out-of-range track is ignored");
    }

    #[test]
    fn a_note_gate_is_its_length_in_frames_and_never_zero() {
        let note = MirroredNote {
            start_beats: 1.0,
            end_beats: 1.5,
            pitch: 60,
            velocity: 100,
        };
        assert_eq!(note.gate_frames(22_050.0), 11_025);
        let blip = MirroredNote {
            end_beats: 1.0,
            ..note
        };
        assert_eq!(blip.gate_frames(22_050.0), 1);
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
    fn a_block_with_an_offset_starts_reading_mid_sample() {
        let song = Song::new();
        let started = |offset_seconds: f32| {
            let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
            let mut frames = vec![0.0f32; 4_096];
            frames.extend_from_slice(&[0.5f32; 4_096]);
            core.push_sample(SampleMsg::Load {
                slot: 0,
                frames: frames.into(),
                rate: DEFAULT_SAMPLE_RATE as f32,
            });
            core.apply(RtCommand::SetBlocks {
                track: 0,
                blocks: Arc::from(vec![MirroredBlock {
                    start_beats: 0.0,
                    end_beats: 4.0,
                    slot: 0,
                    gain: 1.0,
                    offset_seconds,
                    rate: 1.0,
                }]),
            });
            core.apply(RtCommand::Play);
            loud(&render(&mut core, 256))
        };
        assert!(!started(0.0), "the top of the sample is silent");
        assert!(
            started(5_000.0 / DEFAULT_SAMPLE_RATE as f32),
            "the offset skipped the silence"
        );
    }

    /// A slot of silence followed by a constant, at `rate`, so where a voice
    /// reads shows as whether the first buffer sounds.
    fn load_step(core: &mut RenderCore, silent: usize, rate: f32) {
        let mut frames = vec![0.0f32; silent];
        frames.extend_from_slice(&[0.5f32; 44_100]);
        core.push_sample(SampleMsg::Load {
            slot: 0,
            frames: frames.into(),
            rate,
        });
        core.apply(RtCommand::SetSampleRate { slot: 0, rate });
    }

    fn one_block(offset_seconds: f32, rate: f32) -> RtCommand {
        RtCommand::SetBlocks {
            track: 0,
            blocks: Arc::from(vec![MirroredBlock {
                start_beats: 0.0,
                end_beats: 16.0,
                slot: 0,
                gain: 1.0,
                offset_seconds,
                rate,
            }]),
        }
    }

    #[test]
    fn an_offset_counts_in_the_file_frames_not_the_engine_frames() {
        let song = Song::new();
        let starts_loud = |offset_frames: f32| {
            let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
            load_step(&mut core, 4_096, 22_050.0);
            core.apply(one_block(offset_frames / 22_050.0, 1.0));
            core.apply(RtCommand::Play);
            loud(&render(&mut core, 64))
        };
        assert!(!starts_loud(3_000.0), "3000 file frames is still silence");
        assert!(starts_loud(5_000.0), "5000 file frames is past the silence");
    }

    #[test]
    fn a_stretched_block_entered_mid_way_reads_at_its_rate() {
        let song = Song::new();
        let starts_loud = |rate: f32| {
            let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
            load_step(&mut core, 8_192, DEFAULT_SAMPLE_RATE as f32);
            core.apply(one_block(0.0, rate));
            let frames_per_beat = core.transport.frames_per_beat();
            core.apply(RtCommand::Seek {
                beats: (6_000.0 / frames_per_beat) as f32,
            });
            core.apply(RtCommand::Play);
            loud(&render(&mut core, 64))
        };
        assert!(!starts_loud(1.0), "6000 frames in is still silence");
        assert!(starts_loud(2.0), "at double rate 6000 frames reads 12000");
    }

    #[test]
    fn a_split_under_a_playing_sample_still_ends_it_at_the_block_end() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        load_slot(&mut core, 44_100 * 8, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 2.0)]),
        });
        core.apply(RtCommand::Play);
        while core.position_beats() < 1.0 {
            render(&mut core, 512);
        }
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 0.5), (0.5, 2.0)]),
        });
        while core.position_beats() < 2.2 {
            render(&mut core, 512);
        }
        assert!(
            !loud(&render(&mut core, 512)),
            "the sample must stop at the end of the split-off block"
        );
    }

    #[test]
    fn removing_the_block_under_a_playing_sample_fades_it_out() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 44_100 * 8, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 8.0)]),
        });
        core.apply(RtCommand::Play);
        render(&mut core, 2_048);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[]),
        });
        render(&mut core, 4_096);
        assert!(!loud(&render(&mut core, 512)));
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
        core.apply(RtCommand::SetLooping { looping: false });
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
        core.apply(RtCommand::SetLooping { looping: false });
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
        core.apply(RtCommand::SetLooping { looping: false });
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
        core.apply(RtCommand::SetLooping { looping: false });
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
        core.apply(RtCommand::SetLooping { looping: false });
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
        let frames_per_beat = transport.frames_per_beat() as u64;
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
    fn looping_wraps_the_playhead_at_the_song_end() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 1.0)]),
        });
        core.apply(RtCommand::Play);
        let loop_frames = (DEFAULT_SAMPLE_RATE / 2) as usize;
        render(&mut core, loop_frames - 100);
        assert_eq!(core.position_frames(), loop_frames as u64 - 100);
        render(&mut core, 300);
        assert_eq!(core.position_frames(), 200, "wrapped exactly at the end");
    }

    #[test]
    fn without_looping_the_playhead_runs_past_the_song_end() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 1.0)]),
        });
        core.apply(RtCommand::Play);
        render(&mut core, DEFAULT_SAMPLE_RATE as usize);
        assert_eq!(core.position_frames(), DEFAULT_SAMPLE_RATE as u64);
    }

    #[test]
    fn a_block_starts_again_after_the_loop_wraps() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 200, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 1.0)]),
        });
        core.apply(RtCommand::Play);
        let loop_frames = (DEFAULT_SAMPLE_RATE / 2) as usize;
        let mut loud_buffers = 0;
        for _ in 0..(loop_frames * 2 / 512 + 2) {
            if render(&mut core, 512).iter().any(|s| s.abs() > 1e-6) {
                loud_buffers += 1;
            }
        }
        assert!(loud_buffers >= 2, "the block must sound on each pass");
    }

    #[test]
    fn beats_keep_frame_accuracy_past_two_to_the_24_frames() {
        let transport = Transport::new(120.0, DEFAULT_SAMPLE_RATE);
        let frames = ((1u64 << 24) + 1) as f64;
        let beats = transport.frames_to_beats(frames);
        assert_eq!(beats, frames / transport.frames_per_beat());
    }

    #[test]
    fn odd_tempo_lands_a_block_within_one_frame_after_ten_minutes() {
        let mut transport = Transport::new(137.0, 44_100);
        transport.play();
        let target = 26_460_000u64;
        for _ in 0..target - 1 {
            transport.tick_frame();
        }
        let block = MirroredBlock {
            start_beats: 1370.0,
            end_beats: 1374.0,
            slot: 0,
            gain: 1.0,
            offset_seconds: 0.0,
            rate: 1.0,
        };
        let from = transport.position_beats_exact();
        let to = transport.beats_after(2);
        assert!(block.starts_in(from, to));
        let exact = f64::from(block.start_beats) * transport.frames_per_beat();
        assert!((exact - target as f64).abs() <= 1.0);
    }

    #[test]
    fn repeated_tempo_changes_do_not_drift_the_position() {
        let mut transport = Transport::new(137.0, 44_100);
        transport.play();
        for _ in 0..1_000_003 {
            transport.tick_frame();
        }
        let before = transport.position_beats_exact();
        for i in 0..10_000 {
            transport.set_bpm(if i % 2 == 0 { 91.3 } else { 137.0 });
        }
        transport.set_bpm(137.0);
        assert!((transport.position_beats_exact() - before).abs() < 1e-6);
    }

    #[test]
    fn set_blocks_keeps_at_most_max_blocks() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        let spans: Vec<(f32, f32)> = (0..MAX_BLOCKS + 10)
            .map(|i| (i as f32, i as f32 + 0.5))
            .collect();
        core.set_blocks(0, blocks(&spans));
        assert_eq!(core.block_count(0), MAX_BLOCKS);
        assert!(core.block(0, MAX_BLOCKS).is_none());
    }

    #[test]
    fn seeking_fades_a_sample_voice_instead_of_cutting_it() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        load_slot(&mut core, 44_100, 0.5);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: blocks(&[(0.0, 8.0)]),
        });
        core.apply(RtCommand::Play);
        render(&mut core, 512);
        core.apply(RtCommand::Stop);
        let out = render(&mut core, 64);
        let first = out[0].abs();
        let last = out[126].abs();
        assert!(first > 0.0, "the voice is still sounding on the next frame");
        assert!(last < first, "and it fades out");
    }

    fn first_loud_frame(out: &[f32]) -> Option<usize> {
        out.iter().position(|s| s.abs() > 1e-6).map(|i| i / 2)
    }

    fn peak(out: &[f32]) -> f32 {
        out.iter().fold(0.0f32, |peak, s| peak.max(s.abs()))
    }

    fn sign_changes(out: &[f32]) -> usize {
        out.chunks(2)
            .map(|frame| frame[0])
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|pair| (pair[0] > 0.0) != (pair[1] > 0.0))
            .count()
    }

    fn metronome_core() -> RenderCore {
        let mut core = RenderCore::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::SetMetronome { on: true });
        core
    }

    #[test]
    fn the_metronome_clicks_on_the_beat_frame_and_is_silent_between() {
        let mut core = metronome_core();
        core.apply(RtCommand::Seek { beats: 0.5 });
        core.apply(RtCommand::Play);
        let beat = core.transport.frames_per_beat().round() as usize;
        let out = render(&mut core, beat - 11_025 + 2_000);
        assert_eq!(
            first_loud_frame(&out).map(|frame| frame + 11_025),
            Some(beat),
            "the click starts on beat one's frame"
        );
        let click = (CLICK_SECONDS * DEFAULT_SAMPLE_RATE as f32) as usize;
        let after = (beat - 11_025 + click) * 2;
        assert!(!loud(&out[after..]), "silence after the click");
        assert!(peak(&out) <= 1.0);
    }

    #[test]
    fn the_downbeat_click_is_higher_and_louder() {
        let mut core = metronome_core();
        core.apply(RtCommand::Play);
        let beat = core.transport.frames_per_beat().round() as usize;
        let click = (CLICK_SECONDS * DEFAULT_SAMPLE_RATE as f32) as usize;
        let out = render(&mut core, beat * 2);
        let downbeat = &out[..click * 2];
        let other = &out[beat * 2..(beat + click) * 2];
        assert!(peak(downbeat) > peak(other));
        assert!(sign_changes(downbeat) > sign_changes(other));
    }

    #[test]
    fn the_metronome_off_is_silent() {
        let mut core = metronome_core();
        core.apply(RtCommand::SetMetronome { on: false });
        core.apply(RtCommand::Play);
        assert!(!loud(&render(&mut core, 50_000)));
    }

    #[test]
    fn a_new_core_never_clicks() {
        let mut core = RenderCore::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::Play);
        assert!(!loud(&render(&mut core, 50_000)));
    }

    #[test]
    fn a_count_in_holds_the_playhead_clicks_then_plays() {
        let mut core = RenderCore::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::Seek { beats: 2.0 });
        core.apply(RtCommand::PlayAfterCountIn { beats: 4 });
        let beat = core.transport.frames_per_beat().round() as usize;
        assert_eq!(core.count_in_frames(), beat as u64 * 4);
        let mut clicks = 0;
        let mut left = core.count_in_frames();
        for _ in 0..4 {
            let out = render(&mut core, beat);
            clicks += usize::from(first_loud_frame(&out) == Some(0));
            assert_eq!(core.position_beats(), 2.0, "the marker holds");
            assert_eq!(core.is_playing(), core.count_in_frames() == 0);
            assert_eq!(core.count_in_frames(), left - beat as u64);
            left = core.count_in_frames();
        }
        assert_eq!(clicks, 4, "one click per pre-roll beat");
        assert_eq!(core.count_in_frames(), 0);
        assert!(core.is_playing());
        render(&mut core, 100);
        assert!(core.position_beats() > 2.0, "the transport runs after");
    }

    #[test]
    fn a_count_in_ends_on_its_exact_frame_inside_a_buffer() {
        let mut core = RenderCore::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::PlayAfterCountIn { beats: 1 });
        let beat = core.transport.frames_per_beat().round() as u64;
        render(&mut core, beat as usize + 300);
        assert_eq!(core.position_frames(), 300);
    }

    #[test]
    fn stop_and_seek_cancel_a_count_in() {
        let mut core = RenderCore::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::PlayAfterCountIn { beats: 4 });
        render(&mut core, 1_000);
        core.apply(RtCommand::Stop);
        assert_eq!(core.count_in_frames(), 0);
        render(&mut core, 2_000);
        assert!(!core.is_playing());
        let out = render(&mut core, 100_000);
        assert!(!loud(&out), "no more pre-roll clicks");
        core.apply(RtCommand::PlayAfterCountIn { beats: 4 });
        core.apply(RtCommand::Seek { beats: 1.0 });
        assert_eq!(core.count_in_frames(), 0);
        render(&mut core, 100_000);
        assert!(!core.is_playing());
    }

    #[test]
    fn a_count_in_of_zero_beats_plays_at_once() {
        let mut core = RenderCore::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::PlayAfterCountIn { beats: 0 });
        assert!(core.is_playing());
        assert_eq!(core.count_in_frames(), 0);
    }

    #[test]
    fn a_beat_grid_finds_rounded_beat_frames() {
        assert_eq!(beat_grid(0, 10.4), (Some(0), 10));
        assert_eq!(beat_grid(10, 10.4), (Some(1), 11));
        assert_eq!(beat_grid(15, 10.4), (None, 6));
    }

    #[test]
    fn a_song_with_effects_builds_and_renders() {
        let mut song = Song::new();
        song.tracks[0].effects = crate::models::EffectKind::ALL
            .iter()
            .map(|kind| crate::models::Effect::new(*kind))
            .collect();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::NoteOn {
            track: 0,
            pitch: 60,
            velocity: 100,
        });
        assert!(loud(&render(&mut core, 512)));
    }

    #[test]
    fn a_block_covers_its_span_and_not_its_end() {
        let block = MirroredBlock {
            start_beats: 2.0,
            end_beats: 6.0,
            slot: 0,
            gain: 1.0,
            offset_seconds: 0.0,
            rate: 1.0,
        };
        assert!(block.covers(2.0));
        assert!(!block.covers(6.0));
    }
}
