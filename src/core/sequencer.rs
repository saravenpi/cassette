//! The step clock. Runs on the audio thread, sample-accurately.
//!
//! The sequencer owns a fixed-size shadow of the UI's pattern and triggers
//! voices on the synth at every step boundary. It never allocates.

use crate::core::synth::SynthEngine;
use crate::core::{MAX_STEPS, MAX_TRACKS};
use crate::models::{Song, Step};
use crate::utils::time::Tempo;

/// A plain-old-data copy of [`Step`], cheap to move through the command queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepData {
    pub active: bool,
    pub pitch: u8,
    pub velocity: u8,
}

impl StepData {
    /// A silent step.
    pub const OFF: StepData = StepData {
        active: false,
        pitch: 60,
        velocity: 100,
    };

    /// Convert from the domain model.
    pub const fn from_step(step: Step) -> Self {
        Self {
            active: step.active,
            pitch: step.pitch,
            velocity: step.velocity,
        }
    }
}

impl From<Step> for StepData {
    fn from(step: Step) -> Self {
        Self::from_step(step)
    }
}

/// Sample-accurate step clock.
pub struct Sequencer {
    steps: [[StepData; MAX_STEPS]; MAX_TRACKS],
    length_steps: usize,
    track_count: usize,
    bpm: f32,
    sample_rate: u32,
    frames_per_step: u64,
    /// Frame offset inside the current step.
    phase: u64,
    step_index: usize,
    position_frames: u64,
    playing: bool,
    looping: bool,
}

impl Sequencer {
    /// Build a sequencer mirroring `song`.
    pub fn new(song: &Song, sample_rate: u32) -> Self {
        let mut sequencer = Self {
            steps: [[StepData::OFF; MAX_STEPS]; MAX_TRACKS],
            length_steps: song.length_steps.min(MAX_STEPS),
            track_count: song.tracks.len().min(MAX_TRACKS),
            bpm: song.bpm(),
            sample_rate,
            frames_per_step: 1,
            phase: 0,
            step_index: 0,
            position_frames: 0,
            playing: false,
            looping: true,
        };
        sequencer.frames_per_step = Tempo::new(song.bpm()).frames_per_step(sample_rate);
        for (ti, track) in song.tracks.iter().enumerate().take(MAX_TRACKS) {
            for (si, step) in track.steps.iter().enumerate().take(MAX_STEPS) {
                sequencer.steps[ti][si] = StepData::from_step(*step);
            }
        }
        sequencer
    }

    /// Current speed in beats per minute.
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Change tempo, preserving the position within the current step.
    pub fn set_bpm(&mut self, bpm: f32) {
        let bpm = bpm.clamp(20.0, 400.0);
        let ratio = self.phase as f64 / self.frames_per_step as f64;
        self.bpm = bpm;
        self.frames_per_step = Tempo::new(bpm).frames_per_step(self.sample_rate);
        self.phase = (ratio * self.frames_per_step as f64).round() as u64;
    }

    /// Frames in one step at the current tempo.
    pub fn frames_per_step(&self) -> u64 {
        self.frames_per_step
    }

    /// Steps in the pattern.
    pub fn length_steps(&self) -> usize {
        self.length_steps
    }

    /// Change the pattern length.
    pub fn set_length(&mut self, length: usize) {
        self.length_steps = length.clamp(1, MAX_STEPS);
        if self.step_index >= self.length_steps {
            self.step_index = 0;
            self.phase = 0;
        }
    }

    /// Change how many tracks are played.
    pub fn set_track_count(&mut self, count: usize) {
        self.track_count = count.min(MAX_TRACKS);
    }

    /// Write a single step.
    pub fn set_step(&mut self, track: usize, step: usize, data: StepData) {
        if track < MAX_TRACKS && step < MAX_STEPS {
            self.steps[track][step] = data;
        }
    }

    /// Silence one track's whole sequence.
    pub fn clear_track(&mut self, track: usize) {
        if track < MAX_TRACKS {
            self.steps[track] = [StepData::OFF; MAX_STEPS];
        }
    }

    /// Silence the whole pattern.
    pub fn clear_all(&mut self) {
        self.steps = [[StepData::OFF; MAX_STEPS]; MAX_TRACKS];
    }

    /// Start (or resume) playback from the current position.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stop and rewind to the top of the pattern.
    pub fn stop(&mut self) {
        self.playing = false;
        self.step_index = 0;
        self.phase = 0;
        self.position_frames = 0;
    }

    /// Jump to a step (rewinding the frame phase).
    pub fn seek(&mut self, step: usize) {
        self.step_index = step.min(self.length_steps.saturating_sub(1));
        self.phase = 0;
        self.position_frames = self.step_index as u64 * self.frames_per_step;
    }

    /// Enable or disable looping at the end of the pattern.
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    /// Whether the transport is running.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Whether the pattern loops at the end.
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// Total frames since the transport last started from the top.
    pub fn position_frames(&self) -> u64 {
        self.position_frames
    }

    /// Step currently sounding.
    pub fn step_index(&self) -> usize {
        self.step_index
    }

    /// Advance exactly one frame, triggering voices at step boundaries.
    pub fn tick_frame(&mut self, synth: &mut SynthEngine) {
        if !self.playing || self.length_steps == 0 {
            return;
        }

        if self.phase == 0 {
            self.trigger_step(synth);
        }

        self.phase += 1;
        self.position_frames += 1;

        if self.phase >= self.frames_per_step {
            self.phase = 0;
            self.step_index += 1;
            if self.step_index >= self.length_steps {
                self.step_index = 0;
                if !self.looping {
                    self.playing = false;
                }
            }
        }
    }

    fn trigger_step(&mut self, synth: &mut SynthEngine) {
        let gate = self.frames_per_step.min(u32::MAX as u64) as u32;
        for track in 0..self.track_count {
            let data = self.steps[track][self.step_index];
            if data.active {
                synth.note_on(track, data.pitch, data.velocity, gate);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DEFAULT_SAMPLE_RATE;
    use crate::models::Song;

    #[test]
    fn mirrors_the_song_pattern() {
        let song = Song::demo();
        let seq = Sequencer::new(&song, DEFAULT_SAMPLE_RATE);
        assert_eq!(seq.length_steps(), song.length_steps);
        assert_eq!(seq.step_index(), 0);
    }

    #[test]
    fn position_advances_one_frame_per_tick() {
        let mut seq = Sequencer::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        let mut synth = SynthEngine::new(DEFAULT_SAMPLE_RATE);
        seq.play();
        for _ in 0..100 {
            seq.tick_frame(&mut synth);
        }
        assert_eq!(seq.position_frames(), 100);
    }

    #[test]
    fn advances_one_step_per_frames_per_step() {
        let mut seq = Sequencer::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        let mut synth = SynthEngine::new(DEFAULT_SAMPLE_RATE);
        seq.set_length(4);
        seq.play();
        let fps = seq.frames_per_step();
        for _ in 0..fps {
            seq.tick_frame(&mut synth);
        }
        assert_eq!(seq.step_index(), 1);
    }

    #[test]
    fn loops_back_to_the_start_when_looping() {
        let mut seq = Sequencer::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        let mut synth = SynthEngine::new(DEFAULT_SAMPLE_RATE);
        seq.set_length(1);
        seq.play();
        let fps = seq.frames_per_step();
        for _ in 0..fps * 3 {
            seq.tick_frame(&mut synth);
        }
        assert!(seq.is_playing());
        assert_eq!(seq.step_index(), 0);
    }

    #[test]
    fn stops_at_the_end_when_not_looping() {
        let mut seq = Sequencer::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        let mut synth = SynthEngine::new(DEFAULT_SAMPLE_RATE);
        seq.set_length(2);
        seq.set_looping(false);
        seq.play();
        let fps = seq.frames_per_step();
        for _ in 0..fps * 2 + 1 {
            seq.tick_frame(&mut synth);
        }
        assert!(!seq.is_playing());
    }

    #[test]
    fn stop_rewinds() {
        let mut seq = Sequencer::new(&Song::new(), DEFAULT_SAMPLE_RATE);
        let mut synth = SynthEngine::new(DEFAULT_SAMPLE_RATE);
        seq.play();
        for _ in 0..500 {
            seq.tick_frame(&mut synth);
        }
        seq.stop();
        assert_eq!(seq.position_frames(), 0);
        assert_eq!(seq.step_index(), 0);
    }
}
