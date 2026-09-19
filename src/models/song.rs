//! The song: tracks, tempo and pattern length.

use crate::models::Track;
use crate::models::pattern::Step;
use crate::models::track::DEFAULT_LENGTH_STEPS;
use crate::utils::math::clamp;
use crate::utils::time::{STEPS_PER_BEAT, Tempo};

/// A whole session. For now a song holds a single pattern.
#[derive(Clone, Debug)]
pub struct Song {
    bpm: f32,
    /// Steps per pattern, `1..=crate::core::MAX_STEPS`.
    pub length_steps: usize,
    pub tracks: Vec<Track>,
}

impl Default for Song {
    fn default() -> Self {
        Self::demo()
    }
}

impl Song {
    /// A blank song with the default track set.
    pub fn new() -> Self {
        let length = DEFAULT_LENGTH_STEPS;
        let tracks = vec![
            Track::new("Kick", crate::models::Instrument::preset("Kick"), length),
            Track::new("Snare", crate::models::Instrument::preset("Snare"), length),
            Track::new("Hat", crate::models::Instrument::preset("Hat"), length),
            Track::new("Bass", crate::models::Instrument::preset("Bass"), length),
            Track::new("Lead", crate::models::Instrument::preset("Lead"), length),
            Track::new("Chords", crate::models::Instrument::preset("Pad"), length),
        ];
        Self {
            bpm: Tempo::default().bpm,
            length_steps: length,
            tracks,
        }
    }

    /// A song pre-loaded with a small loop so the app makes sound immediately.
    pub fn demo() -> Self {
        let mut song = Self::new();
        song.bpm = 110.0;

        // (track, step, pitch, velocity)
        let notes: &[(usize, usize, u8, u8)] = &[
            // Kick — four on the floor.
            (0, 0, 36, 118),
            (0, 4, 36, 110),
            (0, 8, 36, 118),
            (0, 12, 36, 110),
            // Snare on the backbeat.
            (1, 4, 60, 104),
            (1, 12, 60, 104),
            // Off-beat hats.
            (2, 2, 72, 58),
            (2, 6, 72, 58),
            (2, 10, 72, 58),
            (2, 14, 72, 58),
            // A minor bass line.
            (3, 0, 45, 104),
            (3, 3, 45, 88),
            (3, 6, 40, 100),
            (3, 8, 45, 104),
            (3, 11, 45, 88),
            (3, 14, 43, 96),
            // Lead melody.
            (4, 0, 76, 92),
            (4, 2, 79, 88),
            (4, 4, 81, 96),
            (4, 7, 79, 84),
            (4, 8, 76, 92),
            (4, 10, 74, 88),
            (4, 12, 76, 92),
            (4, 14, 81, 96),
            // Chord stabs.
            (5, 0, 57, 74),
            (5, 4, 53, 70),
            (5, 8, 48, 74),
            (5, 12, 55, 70),
        ];

        for &(track, step, pitch, velocity) in notes {
            if let Some(t) = song.tracks.get_mut(track)
                && let Some(s) = t.steps.get_mut(step)
            {
                *s = Step::on(pitch, velocity);
            }
        }
        song
    }

    /// Current tempo in beats per minute.
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Set the tempo, clamped to a usable range.
    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = clamp(bpm, 20.0, 400.0);
    }

    /// Change the pattern length, resizing every track.
    pub fn set_length(&mut self, length: usize) {
        let length = length.clamp(1, crate::core::MAX_STEPS);
        self.length_steps = length;
        for track in &mut self.tracks {
            track.set_length(length);
        }
    }

    /// Whether any track is soloed.
    pub fn any_solo(&self) -> bool {
        self.tracks.iter().any(|t| t.solo)
    }

    /// Read a step, returning a rest for out-of-range coordinates.
    pub fn step(&self, track: usize, step: usize) -> Step {
        self.tracks
            .get(track)
            .and_then(|t| t.steps.get(step))
            .copied()
            .unwrap_or(Step::EMPTY)
    }

    /// Write a step, ignoring out-of-range coordinates.
    pub fn set_step(&mut self, track: usize, step: usize, value: Step) {
        if let Some(t) = self.tracks.get_mut(track)
            && let Some(s) = t.steps.get_mut(step)
        {
            *s = value;
        }
    }

    /// Flip a step between sounding and resting.
    pub fn toggle_step(&mut self, track: usize, step: usize) {
        if let Some(t) = self.tracks.get_mut(track)
            && let Some(s) = t.steps.get_mut(step)
        {
            s.toggle();
        }
    }

    /// Frames in one step at the given sample rate.
    pub fn frames_per_step(&self, sample_rate: u32) -> u64 {
        Tempo::new(self.bpm).frames_per_step(sample_rate)
    }

    /// Frames in one full pass of the pattern.
    pub fn frames_per_loop(&self, sample_rate: u32) -> u64 {
        self.frames_per_step(sample_rate) * self.length_steps as u64
    }

    /// The step index (0-based) at a given transport position.
    pub fn current_step(&self, position_frames: u64, sample_rate: u32) -> usize {
        let fps = self.frames_per_step(sample_rate);
        if fps == 0 || self.length_steps == 0 {
            return 0;
        }
        ((position_frames / fps) as usize) % self.length_steps
    }

    /// Steps per beat, exposed for the UI ruler.
    pub const fn steps_per_beat() -> u32 {
        STEPS_PER_BEAT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_has_a_full_pattern() {
        let song = Song::demo();
        assert_eq!(song.tracks.len(), 6);
        let sounding: usize = song
            .tracks
            .iter()
            .map(|t| t.steps.iter().filter(|s| s.active).count())
            .sum();
        assert!(sounding > 10, "demo should contain a real pattern");
    }

    #[test]
    fn demo_tracks_fit_the_pattern_length() {
        let song = Song::demo();
        for track in &song.tracks {
            assert_eq!(track.steps.len(), song.length_steps);
        }
    }

    #[test]
    fn toggle_flips_only_the_target_step() {
        let mut song = Song::new();
        song.toggle_step(0, 3);
        assert!(song.step(0, 3).active);
        assert!(!song.step(0, 4).active);
        song.toggle_step(0, 3);
        assert!(!song.step(0, 3).active);
    }

    #[test]
    fn changing_length_resizes_tracks() {
        let mut song = Song::demo();
        song.set_length(32);
        assert_eq!(song.length_steps, 32);
        assert!(song.tracks.iter().all(|t| t.steps.len() == 32));
    }

    #[test]
    fn current_step_tracks_transport_position() {
        let song = Song::demo();
        let fps = song.frames_per_step(44_100);
        assert_eq!(song.current_step(0, 44_100), 0);
        assert_eq!(song.current_step(fps * 3, 44_100), 3);
        // Wraps around the pattern.
        assert_eq!(
            song.current_step(fps * (song.length_steps as u64 + 2), 44_100),
            2
        );
    }

    #[test]
    fn bpm_is_clamped() {
        let mut song = Song::new();
        song.set_bpm(9_999.0);
        assert_eq!(song.bpm(), 400.0);
    }
}
