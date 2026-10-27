//! A track: an instrument, a mixer strip, and the blocks it carries.

use serde::{Deserialize, Serialize};

use super::{Effect, MAX_EFFECTS};

use crate::models::{Instrument, MAX_NOTES_PER_BLOCK, Note};

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
/// A MIDI block carries notes; an audio block points at a file
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
    /// Notes of a MIDI block, in the order they were drawn. Empty for an audio
    /// block.
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Where an audio block starts reading its file, in seconds of source
    /// material. Zero for a MIDI block.
    #[serde(default)]
    pub offset_seconds: f32,
    /// Playback speed of an audio block's material; 2.0 plays twice as fast
    /// and an octave up. One for a MIDI block.
    #[serde(default = "unit_rate")]
    pub rate: f32,
}

fn unit_rate() -> f32 {
    1.0
}

impl Block {
    /// Shortest block the user can drag out.
    pub const MIN_LENGTH_BEATS: f32 = 0.25;
    /// Length a freshly created block gets.
    pub const DEFAULT_LENGTH_BEATS: f32 = 4.0;
    /// Slowest an audio block's material may be stretched to play.
    pub const MIN_RATE: f32 = 0.25;
    /// Fastest an audio block's material may be squeezed to play.
    pub const MAX_RATE: f32 = 4.0;

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
            notes: Vec::new(),
            offset_seconds: 0.0,
            rate: 1.0,
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

    /// Cut the block at `at_beats` on the timeline, keeping the left part and
    /// returning the right one unnamed for the lane to name.
    ///
    /// Returns `None`, changing nothing, unless both parts would be at least
    /// [`Self::MIN_LENGTH_BEATS`] long. The right part of an audio block reads
    /// its material from where the cut fell, `seconds_per_beat` turning beats
    /// into seconds; a MIDI block hands over the notes that start at or after
    /// the cut and shortens the ones that cross it.
    pub fn split_off(&mut self, at_beats: f32, seconds_per_beat: f32) -> Option<Block> {
        let into = at_beats - self.start_beats;
        let rest = self.length_beats - into;
        if !(into >= Self::MIN_LENGTH_BEATS && rest >= Self::MIN_LENGTH_BEATS) {
            return None;
        }
        let (left, right): (Vec<Note>, Vec<Note>) = std::mem::take(&mut self.notes)
            .into_iter()
            .partition(|note| note.start_beats < into);
        self.notes = left
            .into_iter()
            .map(|note| Note {
                length_beats: note.length_beats.min(into - note.start_beats),
                ..note
            })
            .collect();
        let offset_seconds = match self.kind {
            TrackKind::Audio => self.offset_seconds + into * seconds_per_beat * self.rate,
            TrackKind::Midi => self.offset_seconds,
        };
        let split = Block {
            name: String::new(),
            kind: self.kind,
            start_beats: at_beats,
            length_beats: rest,
            source: self.source.clone(),
            notes: right
                .into_iter()
                .map(|note| Note {
                    start_beats: note.start_beats - into,
                    ..note
                })
                .collect(),
            offset_seconds,
            rate: self.rate,
        };
        self.length_beats = into;
        Some(split)
    }

    /// Move the block's left edge to `start_beats`, keeping its end and leaving
    /// its material where it sits on the timeline.
    ///
    /// The block stays at least [`Self::MIN_LENGTH_BEATS`] long and starts on
    /// or after the top of the song. An audio block cannot reach back past the
    /// start of its material; a MIDI block drops the notes the new edge leaves
    /// behind and shortens the ones it cuts.
    pub fn trim_start(&mut self, start_beats: f32, seconds_per_beat: f32) {
        let end = self.end_beats();
        let mut start =
            finite_or(start_beats, self.start_beats).clamp(0.0, end - Self::MIN_LENGTH_BEATS);
        let material_per_beat = seconds_per_beat * self.rate;
        if self.kind == TrackKind::Audio && material_per_beat > 0.0 {
            start = start.max(self.start_beats - self.offset_seconds / material_per_beat);
        }
        let delta = start - self.start_beats;
        match self.kind {
            TrackKind::Audio => {
                self.offset_seconds = (self.offset_seconds + delta * material_per_beat).max(0.0);
            }
            TrackKind::Midi => {
                self.notes
                    .retain(|note| note.start_beats + note.length_beats > delta);
                for note in &mut self.notes {
                    let shifted = note.start_beats - delta;
                    note.length_beats += shifted.min(0.0);
                    note.start_beats = shifted.max(0.0);
                }
            }
        }
        self.start_beats = start;
        self.length_beats = end - start;
    }

    /// Fit an audio block's material to `length_beats` by changing how fast it
    /// plays, within [`Self::MIN_RATE`] and [`Self::MAX_RATE`]. A MIDI block
    /// has no material to stretch, so it is resized instead.
    pub fn stretch_to(&mut self, length_beats: f32) {
        let wanted = clamp_length(length_beats);
        if self.kind == TrackKind::Midi {
            self.length_beats = wanted;
            return;
        }
        let material = self.length_beats * self.rate;
        let rate = (material / wanted).clamp(Self::MIN_RATE, Self::MAX_RATE);
        self.rate = rate;
        self.length_beats = clamp_length(material / rate);
    }
}

fn unit_rate_or(rate: f32) -> f32 {
    if rate.is_finite() && rate > 0.0 {
        rate.clamp(Block::MIN_RATE, Block::MAX_RATE)
    } else {
        1.0
    }
}

fn clamp_start(beats: f32) -> f32 {
    finite_or(beats, 0.0).clamp(0.0, MAX_START_BEATS)
}

fn clamp_length(beats: f32) -> f32 {
    finite_or(beats, Block::DEFAULT_LENGTH_BEATS).clamp(Block::MIN_LENGTH_BEATS, MAX_LENGTH_BEATS)
}

/// A single track of the song.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Insert effects, in signal order.
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub color_index: Option<u8>,
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
            effects: Vec::new(),
            color_index: None,
            next_block_number: 1,
        }
    }

    pub fn with_color(mut self, color_index: u8) -> Self {
        self.color_index = Some(color_index);
        self
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
        let name = self.next_name();
        self.insert_block(Block::new(name, self.kind, start_beats, length_beats, ""))
    }

    /// The next automatic block name on this lane, never one handed out before.
    pub fn next_name(&mut self) -> String {
        let name = format!("{}", self.next_block_number);
        self.next_block_number = self.next_block_number.saturating_add(1);
        name
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
        if self
            .blocks
            .iter()
            .any(|b| b.notes.iter().any(|note| !note.is_finite()))
        {
            return Err(format!("track {:?} holds a non-finite note", self.name));
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
        if let Some(color) = self.color_index {
            self.color_index = Some(color % 8);
        }
        if before != (self.volume, self.pan, *instrument) {
            problems.push(format!("track {:?}: mixer values clamped", self.name));
        }
        let mut clamped = 0;
        for block in &mut self.blocks {
            let before = (
                block.start_beats,
                block.length_beats,
                block.offset_seconds,
                block.rate,
            );
            block.set_start(before.0);
            block.set_length(before.1);
            block.offset_seconds = finite_or(before.2, 0.0).max(0.0);
            block.rate = unit_rate_or(before.3);
            let after = (
                block.start_beats,
                block.length_beats,
                block.offset_seconds,
                block.rate,
            );
            if before != after {
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
        let (mut cleared, mut dropped, mut clamped) = (0, 0, 0);
        for block in &mut self.blocks {
            if block.kind == TrackKind::Audio {
                cleared += usize::from(!block.notes.is_empty());
                block.notes.clear();
                continue;
            }
            dropped += block.notes.len().saturating_sub(MAX_NOTES_PER_BLOCK);
            block.notes.truncate(MAX_NOTES_PER_BLOCK);
            for note in &mut block.notes {
                let before = *note;
                note.clamp_to(MAX_LENGTH_BEATS);
                clamped += usize::from(before != *note);
            }
        }
        if cleared > 0 {
            problems.push(format!(
                "track {:?}: notes removed from {cleared} audio block(s)",
                self.name
            ));
        }
        if dropped > 0 {
            problems.push(format!(
                "track {:?}: dropped {dropped} note(s) past the limit of {MAX_NOTES_PER_BLOCK} per block",
                self.name
            ));
        }
        if clamped > 0 {
            problems.push(format!(
                "track {:?}: {clamped} note(s) clamped to the allowed range",
                self.name
            ));
        }
        problems.extend(self.sanitize_effects());
        Ok(problems)
    }

    /// Keep at most [`MAX_EFFECTS`] effects, with every parameter inside its spec.
    fn sanitize_effects(&mut self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.effects.len() > MAX_EFFECTS {
            let dropped = self.effects.len() - MAX_EFFECTS;
            self.effects.truncate(MAX_EFFECTS);
            problems.push(format!(
                "track {:?}: dropped {dropped} effect(s) past the limit of {MAX_EFFECTS}",
                self.name
            ));
        }
        let clamped = self
            .effects
            .iter_mut()
            .map(Effect::clamp_params)
            .filter(|&changed| changed)
            .count();
        if clamped > 0 {
            problems.push(format!(
                "track {:?}: {clamped} effect(s) clamped to their ranges",
                self.name
            ));
        }
        problems
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
    fn sanitize_brings_offset_and_rate_back_in_range() {
        let mut track = Track::new("a", TrackKind::Audio, Instrument::preset("Lead"));
        for (offset_seconds, rate) in [(-1.0, 9.0), (f32::NAN, f32::NAN), (2.0, 0.0)] {
            let mut block = Block::new("x", TrackKind::Audio, 0.0, 4.0, "/tmp/a.wav");
            block.offset_seconds = offset_seconds;
            block.rate = rate;
            track.blocks.push(block);
        }
        let problems = track.sanitize(8).expect("finite positions");
        let fixed: Vec<(f32, f32)> = track
            .blocks
            .iter()
            .map(|block| (block.offset_seconds, block.rate))
            .collect();
        assert_eq!(fixed, vec![(0.0, Block::MAX_RATE), (0.0, 1.0), (2.0, 1.0)]);
        assert_eq!(problems.len(), 1);
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
                notes: Vec::new(),
                offset_seconds: 0.0,
                rate: 1.0,
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
    fn sanitize_keeps_duplicate_effects_and_clamps_params() {
        use crate::models::EffectKind;
        let mut track = midi();
        let mut reverb = Effect::new(EffectKind::Reverb);
        reverb.params[2] = f32::NAN;
        let mut delay = Effect::new(EffectKind::Delay);
        delay.params[1] = 4.0;
        track.effects = vec![
            reverb,
            Effect::new(EffectKind::Reverb),
            delay,
            Effect::new(EffectKind::Eq),
        ];
        let problems = track.sanitize(8).expect("a non-finite param is fixed");
        let kinds: Vec<EffectKind> = track.effects.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EffectKind::Reverb,
                EffectKind::Reverb,
                EffectKind::Delay,
                EffectKind::Eq
            ]
        );
        assert_eq!(track.effects[0].params[2], 0.25);
        assert_eq!(track.effects[2].params[1], 0.95);
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn sanitize_rejects_non_finite_numbers() {
        let mut track = midi();
        track.volume = f32::NAN;
        assert!(track.sanitize(8).is_err());
    }

    fn note(start_beats: f32, length_beats: f32) -> Note {
        Note {
            pitch: 60,
            start_beats,
            length_beats,
            velocity: 100,
        }
    }

    #[test]
    fn sanitize_clamps_caps_and_clears_notes() {
        let mut track = midi();
        let mut keys = Block::new("keys", TrackKind::Midi, 0.0, 4.0, "");
        keys.notes = vec![note(0.0, 1.0); MAX_NOTES_PER_BLOCK + 3];
        keys.notes[0] = Note {
            pitch: 255,
            start_beats: -1.0,
            length_beats: 0.0,
            velocity: 0,
        };
        keys.notes[1] = note(8.0, 1.0);
        let mut drums = Block::new("drums", TrackKind::Audio, 8.0, 4.0, "/tmp/d.wav");
        drums.notes = vec![note(0.0, 1.0)];
        track.blocks = vec![keys, drums];
        let problems = track.sanitize(8).expect("finite track");
        let keys = &track.blocks[0];
        assert_eq!(keys.notes.len(), MAX_NOTES_PER_BLOCK);
        assert_eq!(
            keys.notes[0],
            Note {
                pitch: 127,
                start_beats: 0.0,
                length_beats: Note::MIN_LENGTH_BEATS,
                velocity: 1,
            }
        );
        assert_eq!(
            keys.notes[1],
            note(8.0, 1.0),
            "a note past a shrunk block's end survives a load"
        );
        assert!(track.blocks[1].notes.is_empty());
        assert_eq!(problems.len(), 3, "{problems:?}");
    }

    #[test]
    fn sanitize_rejects_non_finite_notes() {
        let mut track = midi();
        let mut keys = Block::new("keys", TrackKind::Midi, 0.0, 4.0, "");
        keys.notes = vec![note(f32::NAN, 1.0)];
        track.blocks = vec![keys];
        assert!(track.sanitize(8).is_err());
    }
}
