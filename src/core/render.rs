//! The heart of the audio path: synth + sequencer + command application.
//!
//! [`RenderCore`] is deliberately free of threads and I/O so it can be driven
//! from a real-time callback *or* offline (see [`crate::core::offline`]).

use crate::core::sequencer::{Sequencer, StepData};
use crate::core::synth::SynthEngine;
use crate::core::MAX_TRACKS;
use crate::models::{Instrument, Song};

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
    SetStep {
        track: u8,
        step: u8,
        data: StepData,
    },
    ClearTrack {
        track: u8,
    },
    ClearAll,
    SetLength {
        steps: u8,
    },
    SetTrackCount {
        count: u8,
    },
    SetBpm {
        bpm: f32,
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
    Seek {
        step: u8,
    },
    SetLooping {
        looping: bool,
    },
}

/// Synthesis + sequencing state owned by exactly one thread.
pub struct RenderCore {
    synth: SynthEngine,
    seq: Sequencer,
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
        let seq = Sequencer::new(song, sample_rate);
        Self {
            synth,
            seq,
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
            } => self.synth.note_on(track as usize, pitch, velocity, u32::MAX),
            RtCommand::NoteOff { track, pitch } => self.synth.note_off(track as usize, pitch),
            RtCommand::AllNotesOff => self.synth.all_notes_off(),
            RtCommand::SetStep { track, step, data } => {
                self.seq.set_step(track as usize, step as usize, data)
            }
            RtCommand::ClearTrack { track } => self.seq.clear_track(track as usize),
            RtCommand::ClearAll => self.seq.clear_all(),
            RtCommand::SetLength { steps } => self.seq.set_length(steps as usize),
            RtCommand::SetTrackCount { count } => {
                let count = count.min(MAX_TRACKS as u8) as usize;
                self.seq.set_track_count(count);
                self.synth.set_track_count(count);
            }
            RtCommand::SetBpm { bpm } => self.seq.set_bpm(bpm),
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
            RtCommand::Play => self.seq.play(),
            RtCommand::Stop => {
                self.seq.stop();
                self.synth.all_notes_off();
            }
            RtCommand::Seek { step } => self.seq.seek(step as usize),
            RtCommand::SetLooping { looping } => self.seq.set_looping(looping),
        }
    }

    /// Render interleaved stereo `out`. Returns the per-track peaks observed.
    pub fn process(&mut self, out: &mut [f32]) -> [f32; MAX_TRACKS] {
        let mut peaks = [0.0f32; MAX_TRACKS];
        let frames = out.len() / 2;
        for frame in 0..frames {
            self.seq.tick_frame(&mut self.synth);
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
        self.seq.position_frames()
    }

    /// Whether the transport is running.
    pub fn is_playing(&self) -> bool {
        self.seq.is_playing()
    }

    /// Whether the pattern loops.
    pub fn is_looping(&self) -> bool {
        self.seq.is_looping()
    }

    /// Step currently sounding.
    pub fn step_index(&self) -> usize {
        self.seq.step_index()
    }

    /// Frames per step at the current tempo.
    pub fn frames_per_step(&self) -> u64 {
        self.seq.frames_per_step()
    }

    /// Mutably borrow the sequencer (used by sync helpers).
    pub fn sequencer_mut(&mut self) -> &mut Sequencer {
        &mut self.seq
    }

    /// Mutably borrow the synth (used by sync helpers).
    pub fn synth_mut(&mut self) -> &mut SynthEngine {
        &mut self.synth
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DEFAULT_SAMPLE_RATE;
    use crate::models::Song;

    fn render(core: &mut RenderCore, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        core.process(&mut out);
        out
    }

    #[test]
    fn silent_before_play() {
        let song = Song::demo();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        let out = render(&mut core, 512);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn demo_pattern_produces_audio_when_playing() {
        let song = Song::demo();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::Play);
        let mut peak = 0.0f32;
        for _ in 0..20 {
            let out = render(&mut core, 512);
            for s in out {
                peak = peak.max(s.abs());
            }
        }
        assert!(peak > 0.01, "expected audible output, got {peak}");
    }

    #[test]
    fn stop_silences_and_rewinds() {
        let song = Song::demo();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::Play);
        render(&mut core, 1000);
        core.apply(RtCommand::Stop);
        assert_eq!(core.position_frames(), 0);
        assert!(!core.is_playing());
    }

    #[test]
    fn a_step_can_be_written_while_running() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetStep {
            track: 0,
            step: 0,
            data: StepData {
                active: true,
                pitch: 60,
                velocity: 127,
            },
        });
        core.apply(RtCommand::Play);
        let mut peak = 0.0f32;
        for _ in 0..10 {
            let out = render(&mut core, 512);
            for s in out {
                peak = peak.max(s.abs());
            }
        }
        assert!(peak > 0.0);
    }
}
