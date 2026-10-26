//! The snapshot the webview draws.
//!
//! [`UiState`] is built once per frame from the song and the audio engine's
//! shared state. It holds no references into `core`, so it crosses the Tauri IPC
//! boundary as plain serializable data.

use serde::{Deserialize, Serialize};

use crate::models::{Note, TrackKind};

/// Session cursor: which block of which track is selected. A block index equal
/// to the track's block count means "no block", the state a fresh track is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursor {
    pub block: usize,
    pub track: usize,
}

/// One block on a track's lane, reduced to what the UI needs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockView {
    pub name: String,
    pub kind: TrackKind,
    /// Start position, in beats from the top of the song.
    pub start_beats: f32,
    /// Length, in beats.
    pub length_beats: f32,
    /// Source file of an audio block; empty for a MIDI block.
    pub source: String,
    /// Whether the cursor sits on this block.
    pub selected: bool,
    /// Notes of a MIDI block, starts relative to the block, in the order they
    /// were drawn so an index names a note. Empty off the selected track.
    pub notes: Vec<Note>,
}

/// A track, reduced to what the UI needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    pub name: String,
    pub kind: TrackKind,
    pub preset: String,
    pub waveform: String,
    /// Voice amplitude, `[0, 1]`, so an editor reads it back instead of
    /// guessing it from the preset name.
    pub gain: f32,
    pub octave: i32,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    /// Smoothed meter level.
    pub level: f32,
    /// The track's blocks, in start order.
    pub blocks: Vec<BlockView>,
    /// Index into the frontend's track palette.
    pub color_index: u8,
}

impl TrackView {
    /// End of the track's last block, in beats.
    pub fn end_beats(&self) -> f32 {
        self.blocks
            .last()
            .map_or(0.0, |block| block.start_beats + block.length_beats)
    }
}

/// Everything worth drawing this frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub bpm: f32,
    pub playing: bool,
    pub looping: bool,
    /// Transport position in beats.
    pub transport_position_beats: f32,
    /// How far the ruler has to reach to hold every block, in beats.
    pub length_beats: f32,
    pub bars: u32,
    /// Beats in one bar, so the ruler and the grid agree.
    pub beats_per_bar: u32,
    pub cursor: Cursor,
    pub octave: i32,
    /// Names of the built-in voices, in menu order, for the VST pane.
    pub presets: Vec<String>,
    pub tracks: Vec<TrackView>,
    pub status: String,
    /// Whether the block-name prompt is open, so the webview routes typing to it.
    pub renaming: bool,
    pub backend: String,
    /// Why the engine chose its backend.
    pub backend_note: String,
    pub sample_rate: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(name: &str, start: f32, length: f32) -> BlockView {
        BlockView {
            name: name.into(),
            kind: TrackKind::Midi,
            start_beats: start,
            length_beats: length,
            source: String::new(),
            selected: false,
            notes: Vec::new(),
        }
    }

    fn track(name: &str, blocks: Vec<BlockView>) -> TrackView {
        TrackView {
            name: name.into(),
            kind: TrackKind::Midi,
            preset: "Lead".into(),
            waveform: "Square".into(),
            gain: 0.45,
            octave: 0,
            attack: 0.01,
            decay: 0.12,
            sustain: 0.6,
            release: 0.15,
            volume: 0.8,
            pan: 0.0,
            muted: false,
            solo: false,
            level: 0.5,
            blocks,
            color_index: 0,
        }
    }

    fn state() -> UiState {
        UiState {
            bpm: 120.0,
            playing: true,
            looping: true,
            transport_position_beats: 5.0,
            length_beats: 32.0,
            bars: 8,
            beats_per_bar: 4,
            cursor: Cursor { track: 0, block: 1 },
            octave: 0,
            presets: crate::models::Instrument::PRESETS
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            tracks: vec![
                track("Midi 1", vec![block("Verse", 0.0, 8.0)]),
                track("Audio 1", vec![]),
            ],
            status: "ready".into(),
            renaming: false,
            backend: "silent".into(),
            backend_note: "silent: no device".into(),
            sample_rate: 44_100,
        }
    }

    #[test]
    fn a_track_reports_where_its_material_ends() {
        let track = state().tracks[0].clone();
        assert_eq!(track.end_beats(), 8.0);
        assert_eq!(state().tracks[1].end_beats(), 0.0);
    }

    #[test]
    fn the_snapshot_crosses_ipc_as_camel_case_json() {
        let json = serde_json::to_value(state()).expect("snapshot should serialize");
        assert_eq!(json["transportPositionBeats"], 5.0);
        assert_eq!(json["tracks"][0]["blocks"][0]["startBeats"], 0.0);
        assert_eq!(json["tracks"][0]["blocks"][0]["lengthBeats"], 8.0);
        assert_eq!(json["tracks"][0]["blocks"][0]["kind"], "midi");
        assert_eq!(json["tracks"][1]["blocks"][0], serde_json::Value::Null);
    }
}
