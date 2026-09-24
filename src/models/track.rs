//! A track: an instrument, a mixer strip, and its blocks.

use serde::{Deserialize, Serialize};

use crate::models::Instrument;

/// What a track carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    /// Plays the built-in synth from note data.
    Midi,
    /// Will carry recorded or imported audio.
    Audio,
}

impl TrackKind {
    /// Short name for the track list.
    pub const fn name(self) -> &'static str {
        match self {
            TrackKind::Midi => "MIDI",
            TrackKind::Audio => "AUDIO",
        }
    }
}

/// One slot on a track. A block holds no content yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: String,
}

impl Block {
    /// Create a block with the given label.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
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
    /// The track's blocks, in column order.
    pub blocks: Vec<Block>,
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
        }
    }

    /// Append an empty block, returning its index.
    pub fn add_block(&mut self) -> usize {
        let index = self.blocks.len();
        self.blocks.push(Block::new(format!("{}", index + 1)));
        index
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
    fn blocks_are_appended_in_order() {
        let mut track = Track::new("t", TrackKind::Midi, Instrument::preset("Lead"));
        assert_eq!(track.add_block(), 0);
        assert_eq!(track.add_block(), 1);
        assert_eq!(track.blocks.len(), 2);
        assert_eq!(track.blocks[1].name, "2");
    }

    #[test]
    fn solo_logic() {
        let mut track = Track::new("t", TrackKind::Midi, Instrument::preset("Lead"));
        assert!(track.is_audible(false));
        track.muted = true;
        assert!(!track.is_audible(false));
        track.muted = false;
        assert!(!track.is_audible(true));
        track.solo = true;
        assert!(track.is_audible(true));
    }
}
