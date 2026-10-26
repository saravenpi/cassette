//! A track: an instrument, a mixer strip, and the blocks it carries.

use serde::{Deserialize, Serialize};

use crate::models::Instrument;

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

/// Furthest a block may start, in beats from the top of the song.
pub const MAX_START_BEATS: f32 = 4096.0;

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// A block placed on a track's lane.
///
/// A MIDI block carries note data (none yet); an audio block points at a file
/// on disk. Both live at a position on the timeline and have a length in beats,
/// so the lane can draw them and the engine can find the one under the playhead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
            start_beats: clamp_start(start_beats),
            length_beats: clamp_length(length_beats),
            source: source.into(),
        }
    }

    /// Position just past the block.
    pub fn end_beats(&self) -> f32 {
        self.start_beats + self.length_beats
    }

    /// Move the block, keeping it on or after the top of the song.
    pub fn set_start(&mut self, beats: f32) {
        self.start_beats = clamp_start(beats);
    }

    /// Resize the block, respecting the minimum length.
    pub fn set_length(&mut self, beats: f32) {
        self.length_beats = clamp_length(beats);
    }
}

fn clamp_start(beats: f32) -> f32 {
    finite_or(beats, 0.0).clamp(0.0, MAX_START_BEATS)
}

fn clamp_length(beats: f32) -> f32 {
    finite_or(beats, Block::DEFAULT_LENGTH_BEATS).clamp(Block::MIN_LENGTH_BEATS, MAX_LENGTH_BEATS)
}

/// A single track of the song.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

    /// Position a new block can take without landing on an existing one: the end
    /// of the last block, or the top of the song when the lane is empty.
    pub fn free_start(&self) -> f32 {
        self.blocks
            .last()
            .map_or(0.0, |block| block.end_beats().ceil())
    }

    /// Bring a deserialized track back inside the limits the app enforces.
    ///
    /// Returns an error for numbers that are not finite, which no clamp can
    /// make meaningful. Otherwise returns one message per kind of correction.
    pub fn sanitize(&mut self, max_blocks: usize) -> Result<Vec<String>, String> {
        let instrument = &mut self.instrument;
        let numbers = [
            self.volume,
            self.pan,
            instrument.attack,
            instrument.decay,
            instrument.sustain,
            instrument.release,
            instrument.gain,
        ];
        if numbers.iter().any(|n| !n.is_finite()) {
            return Err(format!("track {:?} holds a non-finite number", self.name));
        }
        if self
            .blocks
            .iter()
            .any(|b| !b.start_beats.is_finite() || !b.length_beats.is_finite())
        {
            return Err(format!("track {:?} holds a non-finite block", self.name));
        }
        let mut problems = Vec::new();
        let before = (self.volume, self.pan, *instrument);
        self.volume = self.volume.clamp(0.0, 1.0);
        self.pan = self.pan.clamp(-1.0, 1.0);
        instrument.gain = instrument.gain.clamp(0.0, 1.0);
        instrument.sustain = instrument.sustain.clamp(0.0, 1.0);
        instrument.attack = instrument.attack.clamp(0.0, 10.0);
        instrument.decay = instrument.decay.clamp(0.0, 10.0);
        instrument.release = instrument.release.clamp(0.0, 10.0);
        instrument.octave = instrument.octave.clamp(-4, 4);
        if before != (self.volume, self.pan, *instrument) {
            problems.push(format!("track {:?}: mixer values clamped", self.name));
        }
        let mut clamped = 0;
        for block in &mut self.blocks {
            let (start, length) = (block.start_beats, block.length_beats);
            block.set_start(start);
            block.set_length(length);
            if (start, length) != (block.start_beats, block.length_beats) {
                clamped += 1;
            }
        }
        if clamped > 0 {
            problems.push(format!(
                "track {:?}: {clamped} block(s) clamped to the allowed range",
                self.name
            ));
        }
        self.blocks
            .sort_by(|a, b| a.start_beats.total_cmp(&b.start_beats));
        if self.blocks.len() > max_blocks {
            let dropped = self.blocks.len() - max_blocks;
            self.blocks.truncate(max_blocks);
            problems.push(format!(
                "track {:?}: dropped {dropped} block(s) past the limit of {max_blocks}",
                self.name
            ));
        }
        Ok(problems)
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
    fn block_new_caps_start_and_length() {
        let block = Block::new("a", TrackKind::Midi, 1e9, 1e9, "");
        assert_eq!(block.start_beats, MAX_START_BEATS);
        assert_eq!(block.length_beats, MAX_LENGTH_BEATS);
        let block = Block::new("a", TrackKind::Midi, f32::NAN, f32::INFINITY, "");
        assert_eq!(block.start_beats, 0.0);
        assert_eq!(block.length_beats, Block::DEFAULT_LENGTH_BEATS);
    }

    #[test]
    fn sanitize_sorts_clamps_and_caps() {
        let mut track = midi();
        track.volume = 5.0;
        track.pan = -9.0;
        for start in [30.0, 10.0, 20.0] {
            track.blocks.push(Block {
                name: "x".into(),
                kind: TrackKind::Midi,
                start_beats: start,
                length_beats: -3.0,
                source: String::new(),
            });
        }
        let problems = track.sanitize(2).expect("finite track");
        assert_eq!(track.volume, 1.0);
        assert_eq!(track.pan, -1.0);
        assert_eq!(track.blocks.len(), 2);
        assert_eq!(track.blocks[0].start_beats, 10.0);
        assert_eq!(track.blocks[0].length_beats, Block::MIN_LENGTH_BEATS);
        assert_eq!(problems.len(), 3);
    }

    #[test]
    fn sanitize_rejects_non_finite_numbers() {
        let mut track = midi();
        track.volume = f32::NAN;
        assert!(track.sanitize(8).is_err());
    }
}
