//! A track: an instrument, a mixer strip, and the blocks it carries.

use serde::{Deserialize, Serialize};

use crate::models::Instrument;
use crate::utils::math::clamp;

/// What a track carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    /// Plays the built-in synth from note data.
    Midi,
    /// Carries audio files on the timeline.
    Audio,
}

impl TrackKind {
    /// Short name for the track header.
    pub const fn name(self) -> &'static str {
        match self {
            TrackKind::Midi => "MIDI",
            TrackKind::Audio => "AUDIO",
        }
    }

    /// The other kind, for the `C` key.
    pub const fn flipped(self) -> Self {
        match self {
            TrackKind::Midi => TrackKind::Audio,
            TrackKind::Audio => TrackKind::Midi,
        }
    }

    /// The instrument a fresh track of this kind starts on.
    pub fn default_preset(self) -> Instrument {
        Instrument::preset(match self {
            TrackKind::Midi => "Lead",
            TrackKind::Audio => "Pluck",
        })
    }
}

/// Longest block the user can drag out, in beats.
pub const MAX_LENGTH_BEATS: f32 = 512.0;

/// A block placed on a track's lane.
///
/// A MIDI block carries note data (none yet); an audio block points at a file
/// on disk. Both live at a position on the timeline and have a length in beats,
/// so the lane can draw them and the engine can find the one under the playhead.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub name: String,
    pub kind: TrackKind,
    /// Start position, in beats from the top of the song.
    pub start_beats: f32,
    /// Length, in beats.
    pub length_beats: f32,
    /// Path of the audio file this block plays. Empty for a MIDI block.
    pub source: String,
}

impl Block {
    /// Shortest block the user can drag out.
    pub const MIN_LENGTH_BEATS: f32 = 0.25;
    /// Length a freshly created block gets.
    pub const DEFAULT_LENGTH_BEATS: f32 = 4.0;

    /// Create a block.
    pub fn new(
        name: impl Into<String>,
        kind: TrackKind,
        start_beats: f32,
        length_beats: f32,
        source: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            start_beats: start_beats.max(0.0),
            length_beats: length_beats.max(Self::MIN_LENGTH_BEATS),
            source: source.into(),
        }
    }

    /// Position just past the block.
    pub fn end_beats(&self) -> f32 {
        self.start_beats + self.length_beats
    }

    /// Whether the block covers `beats`.
    pub fn contains(&self, beats: f32) -> bool {
        beats >= self.start_beats && beats < self.end_beats()
    }

    /// Move the block, keeping it on or after the top of the song.
    pub fn set_start(&mut self, beats: f32) {
        self.start_beats = beats.max(0.0);
    }

    /// Resize the block, respecting the minimum length.
    pub fn set_length(&mut self, beats: f32) {
        self.length_beats = clamp(beats, Self::MIN_LENGTH_BEATS, MAX_LENGTH_BEATS);
    }
}

/// A single track of the song.
#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    pub instrument: Instrument,
    /// Linear gain, `[0, 1]`.
    pub volume: f32,
    /// Stereo position, `-1.0` (left) to `1.0` (right).
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    /// The track's blocks, kept in start order.
    pub blocks: Vec<Block>,
    /// Numbers handed to new blocks. It only ever rises, so a lane's automatic
    /// names stay unique even after a block is deleted.
    next_block_number: u32,
}

impl Track {
    /// Create an empty track of the given kind.
    pub fn new(name: impl Into<String>, kind: TrackKind, instrument: Instrument) -> Self {
        Self {
            name: name.into(),
            kind,
            instrument,
            volume: 0.8,
            pan: 0.0,
            muted: false,
            solo: false,
            blocks: Vec::new(),
            next_block_number: 1,
        }
    }

    /// Insert a block, keeping `blocks` sorted by start position.
    pub fn insert_block(&mut self, block: Block) -> usize {
        let index = self
            .blocks
            .partition_point(|existing| existing.start_beats <= block.start_beats);
        self.blocks.insert(index, block);
        index
    }

    /// Create a block of this track's kind at `start_beats`, returning its index.
    ///
    /// The name comes from a counter rather than the block count, so no two
    /// blocks on one lane share an automatic name: the interface tells blocks
    /// apart by name while a drag is in flight.
    pub fn add_block(&mut self, start_beats: f32, length_beats: f32) -> usize {
        let name = format!("{}", self.next_block_number);
        self.next_block_number = self.next_block_number.saturating_add(1);
        self.insert_block(Block::new(name, self.kind, start_beats, length_beats, ""))
    }

    /// The index of the block covering `beats`, if any.
    pub fn block_at(&self, beats: f32) -> Option<usize> {
        self.blocks.iter().position(|block| block.contains(beats))
    }

    /// Position a new block can take without landing on an existing one: the end
    /// of the last block, or the top of the song when the lane is empty.
    pub fn free_start(&self) -> f32 {
        self.blocks
            .last()
            .map_or(0.0, |block| block.end_beats().ceil())
    }

    /// Whether this track produces sound given the global solo state.
    pub fn is_audible(&self, any_solo: bool) -> bool {
        !self.muted && (!any_solo || self.solo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn midi() -> Track {
        Track::new("t", TrackKind::Midi, Instrument::preset("Lead"))
    }

    #[test]
    fn blocks_are_appended_in_order() {
        let mut track = midi();
        assert_eq!(track.add_block(0.0, 4.0), 0);
        assert_eq!(track.add_block(4.0, 4.0), 1);
        assert_eq!(track.blocks.len(), 2);
        assert_eq!(track.blocks[1].name, "2");
    }

    #[test]
    fn blocks_stay_sorted_by_start() {
        let mut track = midi();
        track.add_block(16.0, 4.0);
        let index = track.add_block(4.0, 4.0);
        assert_eq!(index, 0, "an earlier block takes the earlier slot");
        assert_eq!(track.blocks[0].start_beats, 4.0);
        assert_eq!(track.blocks[1].start_beats, 16.0);
    }

    #[test]
    fn a_block_covers_its_span_and_not_its_end() {
        let block = Block::new("a", TrackKind::Midi, 2.0, 4.0, "");
        assert!(block.contains(2.0));
        assert!(block.contains(5.9));
        assert!(!block.contains(6.0), "the end belongs to the next block");
        assert!(!block.contains(1.9));
    }

    #[test]
    fn the_track_finds_the_block_under_a_position() {
        let mut track = midi();
        track.add_block(0.0, 4.0);
        track.add_block(8.0, 4.0);
        assert_eq!(track.block_at(1.0), Some(0));
        assert_eq!(track.block_at(9.0), Some(1));
        assert_eq!(track.block_at(6.0), None);
    }

    #[test]
    fn a_new_block_lands_past_the_last_one() {
        let mut track = midi();
        assert_eq!(track.free_start(), 0.0);
        track.add_block(0.0, 4.0);
        assert_eq!(track.free_start(), 4.0);
    }

    #[test]
    fn a_block_never_goes_below_its_minimum_length() {
        let mut block = Block::new("a", TrackKind::Midi, 0.0, 4.0, "");
        block.set_length(0.0);
        assert_eq!(block.length_beats, Block::MIN_LENGTH_BEATS);
    }

    #[test]
    fn automatic_block_names_never_repeat_on_a_lane() {
        let mut track = midi();
        track.add_block(0.0, 4.0);
        track.add_block(4.0, 4.0);
        track.blocks.remove(0);
        track.add_block(8.0, 4.0);
        let mut names: Vec<&str> = track
            .blocks
            .iter()
            .map(|block| block.name.as_str())
            .collect();
        names.sort_unstable();
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(names, unique, "a deleted block's name must not come back");
        assert_eq!(names, vec!["2", "3"]);
    }

    #[test]
    fn solo_logic() {
        let mut track = midi();
        assert!(track.is_audible(false));
        track.muted = true;
        assert!(!track.is_audible(false));
        track.muted = false;
        assert!(!track.is_audible(true));
        track.solo = true;
        assert!(track.is_audible(true));
    }
}
