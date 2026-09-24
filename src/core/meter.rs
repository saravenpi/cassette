//! Metering (Phase 4.2).
//!
//! [`MeterBank`] smooths the peaks published by the audio thread into the
//! ballistics a mixer needs (fast attack, slow decay, peak hold).

use crate::core::MAX_TRACKS;

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn bank_reports_one_meter_per_track() {
        let mut bank = MeterBank::new(4);
        let mut peaks = [0.0f32; MAX_TRACKS];
        peaks[1] = 0.5;
        bank.update(&peaks);
        assert_eq!(bank.count(), 4);
        assert!(bank.level(1) > 0.0);
        assert_eq!(bank.level(4), 0.0);
    }
}
