//! Tempo and time conversion.

use crate::utils::math::clamp;

/// Number of sequencer steps per quarter note. Steps are 16th notes.
pub const STEPS_PER_BEAT: u32 = 4;

/// The single authority for converting between musical time and samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tempo {
    /// Quarter notes per minute.
    pub bpm: f32,
}

impl Default for Tempo {
    fn default() -> Self {
        Self::new(120.0)
    }
}

impl Tempo {
    /// Create a tempo, clamped to a musically sane range.
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm: clamp(bpm, 20.0, 400.0),
        }
    }

    /// Duration of one beat (quarter note) in seconds.
    pub fn beat_duration(&self) -> f32 {
        60.0 / self.bpm
    }

    /// Duration of one sequencer step in seconds.
    pub fn seconds_per_step(&self) -> f32 {
        self.beat_duration() / STEPS_PER_BEAT as f32
    }

    /// Number of audio frames in one sequencer step. Always at least one.
    pub fn frames_per_step(&self, sample_rate: u32) -> u64 {
        (sample_rate as f32 * self.seconds_per_step()).round().max(1.0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_per_step_at_120bpm_44k() {
        let tempo = Tempo::new(120.0);
        // 120 BPM -> 0.5 s/beat -> 0.125 s/step -> 5512.5 frames @ 44.1 kHz.
        assert_eq!(tempo.frames_per_step(44_100), 5513);
    }

    #[test]
    fn bpm_is_clamped() {
        assert_eq!(Tempo::new(1_000.0).bpm, 400.0);
        assert_eq!(Tempo::new(0.0).bpm, 20.0);
    }

    #[test]
    fn faster_tempo_means_shorter_steps() {
        assert!(
            Tempo::new(180.0).frames_per_step(44_100)
                < Tempo::new(90.0).frames_per_step(44_100)
        );
    }
}
