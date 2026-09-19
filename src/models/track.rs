//! A track: one instrument plus its sequence of steps.

use crate::models::Instrument;
use crate::models::pattern::Step;

/// Default number of steps in a pattern.
pub const DEFAULT_LENGTH_STEPS: usize = 16;

/// A single track of the song.
#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub instrument: Instrument,
    /// Linear gain, `[0, 1]`.
    pub volume: f32,
    /// Stereo position, `-1.0` (left) to `1.0` (right).
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    /// One step per pattern position. Always `song.length_steps` long.
    pub steps: Vec<Step>,
}

impl Track {
    /// Create a track with an empty sequence of `length` steps.
    pub fn new(name: impl Into<String>, instrument: Instrument, length: usize) -> Self {
        Self {
            name: name.into(),
            instrument,
            volume: 0.8,
            pan: 0.0,
            muted: false,
            solo: false,
            steps: vec![Step::EMPTY; length.max(1)],
        }
    }

    /// Grow or shrink the sequence, preserving existing steps.
    pub fn set_length(&mut self, length: usize) {
        self.steps.resize(length.max(1), Step::EMPTY);
    }

    /// Whether this track produces sound given the global solo state.
    pub fn is_audible(&self, any_solo: bool) -> bool {
        !self.muted && (!any_solo || self.solo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_preserves_steps() {
        let mut track = Track::new("t", Instrument::preset("Lead"), 4);
        track.steps[0] = Step::on(64, 100);
        track.set_length(8);
        assert_eq!(track.steps.len(), 8);
        assert_eq!(track.steps[0].pitch, 64);
        track.set_length(2);
        assert_eq!(track.steps.len(), 2);
    }

    #[test]
    fn solo_logic() {
        let mut track = Track::new("t", Instrument::preset("Lead"), 4);
        assert!(track.is_audible(false));
        track.muted = true;
        assert!(!track.is_audible(false));
        track.muted = false;
        assert!(!track.is_audible(true));
        track.solo = true;
        assert!(track.is_audible(true));
    }
}
