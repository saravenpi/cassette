//! Tempo and time conversion.

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
            bpm: bpm.clamp(20.0, 400.0),
        }
    }

    /// Duration of one beat (quarter note) in seconds.
    pub fn beat_duration(&self) -> f32 {
        60.0 / self.bpm
    }

    /// Number of audio frames in one beat, fractional so odd tempos never drift.
    /// Always at least one.
    pub fn frames_per_beat(&self, sample_rate: u32) -> f64 {
        (f64::from(sample_rate) * 60.0 / f64::from(self.bpm)).max(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_per_beat_at_120bpm_44k() {
        let tempo = Tempo::new(120.0);
        // 120 BPM -> 0.5 s/beat -> 22050 frames @ 44.1 kHz.
        assert_eq!(tempo.frames_per_beat(44_100), 22_050.0);
    }

    #[test]
    fn non_integer_beats_keep_their_fraction() {
        let frames = Tempo::new(137.0).frames_per_beat(44_100);
        assert!((frames - 44_100.0 * 60.0 / 137.0).abs() < 1e-9);
        assert!(frames.fract() > 0.0);
    }

    #[test]
    fn bpm_is_clamped() {
        assert_eq!(Tempo::new(1_000.0).bpm, 400.0);
        assert_eq!(Tempo::new(0.0).bpm, 20.0);
    }

    #[test]
    fn faster_tempo_means_shorter_beats() {
        assert!(
            Tempo::new(180.0).frames_per_beat(44_100) < Tempo::new(90.0).frames_per_beat(44_100)
        );
    }
}
