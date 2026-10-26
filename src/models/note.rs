//! A note inside a MIDI block.

use serde::{Deserialize, Serialize};

/// Most notes one MIDI block holds.
pub const MAX_NOTES_PER_BLOCK: usize = 512;

/// One note of a MIDI block. Its start is relative to the block's start, so a
/// block carries its notes along when it moves.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub pitch: u8,
    /// Start, in beats from the start of the block.
    pub start_beats: f32,
    /// Length, in beats.
    pub length_beats: f32,
    pub velocity: u8,
}

impl Note {
    /// Velocity a freshly drawn note gets.
    pub const DEFAULT_VELOCITY: u8 = 100;
    /// Length a freshly drawn note gets.
    pub const DEFAULT_LENGTH_BEATS: f32 = 1.0;
    /// Shortest note the user can draw.
    pub const MIN_LENGTH_BEATS: f32 = 0.0625;
    /// Highest MIDI pitch.
    pub const MAX_PITCH: u8 = 127;
    /// Softest velocity that still sounds.
    pub const MIN_VELOCITY: u8 = 1;
    /// Hardest MIDI velocity.
    pub const MAX_VELOCITY: u8 = 127;

    /// Whether both positions are finite numbers.
    pub fn is_finite(&self) -> bool {
        self.start_beats.is_finite() && self.length_beats.is_finite()
    }

    /// Bring the note inside a span of `limit_beats`: a MIDI pitch, an audible
    /// velocity, a start inside the span and an end at or before the span's end.
    /// A start that is not finite falls back to `0`, a length to the default.
    pub fn clamp_to(&mut self, limit_beats: f32) {
        self.pitch = self.pitch.min(Self::MAX_PITCH);
        self.velocity = self.velocity.clamp(Self::MIN_VELOCITY, Self::MAX_VELOCITY);
        let start = if self.start_beats.is_finite() {
            self.start_beats
        } else {
            0.0
        };
        let length = if self.length_beats.is_finite() {
            self.length_beats
        } else {
            Self::DEFAULT_LENGTH_BEATS
        };
        let last_start = (limit_beats - Self::MIN_LENGTH_BEATS).max(0.0);
        self.start_beats = start.clamp(0.0, last_start);
        let longest = (limit_beats - self.start_beats).max(Self::MIN_LENGTH_BEATS);
        self.length_beats = length.clamp(Self::MIN_LENGTH_BEATS, longest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(pitch: u8, start_beats: f32, length_beats: f32, velocity: u8) -> Note {
        Note {
            pitch,
            start_beats,
            length_beats,
            velocity,
        }
    }

    #[test]
    fn clamping_keeps_a_note_inside_its_span() {
        let mut wild = note(200, 9.0, 9.0, 0);
        wild.clamp_to(4.0);
        assert_eq!(
            wild,
            note(127, 4.0 - Note::MIN_LENGTH_BEATS, Note::MIN_LENGTH_BEATS, 1)
        );

        let mut early = note(60, -2.0, 0.0, 255);
        early.clamp_to(4.0);
        assert_eq!(early, note(60, 0.0, Note::MIN_LENGTH_BEATS, 127));

        let mut long = note(60, 3.0, 8.0, 100);
        long.clamp_to(4.0);
        assert_eq!(long.length_beats, 1.0);
    }

    #[test]
    fn clamping_replaces_numbers_that_are_not_finite() {
        let mut broken = note(60, f32::NAN, f32::INFINITY, 100);
        assert!(!broken.is_finite());
        broken.clamp_to(4.0);
        assert!(broken.is_finite());
        assert_eq!(broken.start_beats, 0.0);
        assert_eq!(broken.length_beats, Note::DEFAULT_LENGTH_BEATS);
    }
}
