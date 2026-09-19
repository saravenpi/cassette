//! Metering and recording (Phase 4.2).
//!
//! Two pieces:
//!
//! * [`MeterBank`] smooths the peaks published by the audio thread into the
//!   ballistics a mixer needs (fast attack, slow decay, peak hold).
//! * [`PerformanceRecorder`] captures live played notes into the pattern,
//!   quantised to the current step — the terminal equivalent of MIDI recording.

use crate::core::MAX_TRACKS;
use crate::models::pattern::Step;
use crate::models::Song;

/// Smoothed level meter with peak hold.
#[derive(Clone, Copy, Debug, Default)]
pub struct LevelMeter {
    level: f32,
    peak: f32,
    hold: u32,
}

impl LevelMeter {
    /// Frames a peak is held before decaying.
    const HOLD_FRAMES: u32 = 30;
    /// Attack coefficient (how fast the meter rises).
    const ATTACK: f32 = 0.45;
    /// Decay coefficient (how slowly the meter falls).
    const DECAY: f32 = 0.08;

    /// Feed one peak measurement.
    pub fn update(&mut self, input: f32) {
        let input = input.clamp(0.0, 1.0);
        if input > self.level {
            self.level += (input - self.level) * Self::ATTACK;
        } else {
            self.level += (input - self.level) * Self::DECAY;
        }

        if input >= self.peak {
            self.peak = input;
            self.hold = Self::HOLD_FRAMES;
        } else if self.hold > 0 {
            self.hold -= 1;
        } else {
            self.peak *= 0.9;
        }
    }

    /// Current smoothed level.
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Current held peak.
    pub fn peak(&self) -> f32 {
        self.peak
    }
}

/// One [`LevelMeter`] per track.
pub struct MeterBank {
    meters: [LevelMeter; MAX_TRACKS],
    count: usize,
}

impl MeterBank {
    /// Create a bank sized for `count` tracks.
    pub fn new(count: usize) -> Self {
        Self {
            meters: [LevelMeter::default(); MAX_TRACKS],
            count: count.min(MAX_TRACKS),
        }
    }

    /// Resize the bank, resetting any new meters.
    pub fn set_count(&mut self, count: usize) {
        self.count = count.min(MAX_TRACKS);
    }

    /// Number of meters in use.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Feed a snapshot of per-track peaks.
    pub fn update(&mut self, peaks: &[f32; MAX_TRACKS]) {
        for (meter, peak) in self.meters.iter_mut().zip(peaks.iter()).take(self.count) {
            meter.update(*peak);
        }
    }

    /// Smoothed level for a track.
    pub fn level(&self, track: usize) -> f32 {
        self.meters.get(track).map_or(0.0, LevelMeter::level)
    }

    /// Held peak for a track.
    pub fn peak(&self, track: usize) -> f32 {
        self.meters.get(track).map_or(0.0, LevelMeter::peak)
    }
}

/// Captures live notes into the pattern while armed.
#[derive(Clone, Copy, Debug, Default)]
pub struct PerformanceRecorder {
    armed: bool,
}

impl PerformanceRecorder {
    /// Create a disarmed recorder.
    pub fn new() -> Self {
        Self { armed: false }
    }

    /// Arm the recorder.
    pub fn arm(&mut self) {
        self.armed = true;
    }

    /// Disarm the recorder.
    pub fn disarm(&mut self) {
        self.armed = false;
    }

    /// Toggle arming, returning the new state.
    pub fn toggle(&mut self) -> bool {
        self.armed = !self.armed;
        self.armed
    }

    /// Whether the recorder is armed.
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Write a played note into `song` at `step`. Returns whether anything was
    /// captured (armed and in range).
    pub fn capture(&self, song: &mut Song, track: usize, step: usize, pitch: u8, velocity: u8) -> bool {
        if !self.armed {
            return false;
        }
        let Some(target) = song.tracks.get_mut(track) else {
            return false;
        };
        let Some(slot) = target.steps.get_mut(step) else {
            return false;
        };
        *slot = Step::on(pitch, velocity);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Song;

    #[test]
    fn meter_rises_fast_and_decays_slowly() {
        let mut meter = LevelMeter::default();
        meter.update(1.0);
        let after_attack = meter.level();
        assert!(after_attack > 0.4);
        for _ in 0..10 {
            meter.update(0.0);
        }
        assert!(meter.level() < after_attack, "meter should decay");
    }

    #[test]
    fn meter_holds_its_peak() {
        let mut meter = LevelMeter::default();
        meter.update(0.8);
        for _ in 0..10 {
            meter.update(0.0);
        }
        assert!((meter.peak() - 0.8).abs() < 1e-6, "peak should be held");
    }

    #[test]
    fn disarmed_recorder_ignores_notes() {
        let mut song = Song::new();
        let recorder = PerformanceRecorder::new();
        assert!(!recorder.capture(&mut song, 0, 0, 60, 100));
        assert!(!song.step(0, 0).active);
    }

    #[test]
    fn armed_recorder_writes_notes() {
        let mut song = Song::new();
        let mut recorder = PerformanceRecorder::new();
        assert!(recorder.toggle());
        assert!(recorder.capture(&mut song, 1, 3, 72, 90));
        assert_eq!(song.step(1, 3).pitch, 72);
        assert!(song.step(1, 3).active);
    }

    #[test]
    fn recorder_ignores_out_of_range_targets() {
        let mut song = Song::new();
        let mut recorder = PerformanceRecorder::new();
        recorder.arm();
        assert!(!recorder.capture(&mut song, 999, 0, 60, 100));
        assert!(!recorder.capture(&mut song, 0, 9_999, 60, 100));
    }
}
