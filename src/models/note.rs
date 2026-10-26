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
}
