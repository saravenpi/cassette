//! The snapshot the webview draws.
//!
//! [`UiState`] is built once per frame from the song and the audio engine's
//! shared state. It holds no references into `core`, so it crosses the Tauri IPC
//! boundary as plain serializable data.

use serde::{Deserialize, Serialize};

use crate::models::TrackKind;

/// Session cursor: which block of which track is selected. A block index equal
/// to the track's block count means the `+` slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursor {
    pub block: usize,
    pub track: usize,
}

/// A track, reduced to what the UI needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    pub name: String,
    pub kind: TrackKind,
    pub preset: String,
    pub waveform: String,
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
    /// Block labels, in column order.
    pub blocks: Vec<String>,
    /// Whether the cursor sits on this track's `+` slot.
    pub add_slot: bool,
    /// Index into the frontend's track palette.
    pub color_index: u8,
}

impl TrackView {
    /// How many blocks the track holds.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
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
    pub cursor: Cursor,
    pub octave: i32,
    pub tracks: Vec<TrackView>,
    pub status: String,
    /// Whether the block-name prompt is open, so the webview routes typing to it.
    pub renaming: bool,
    pub backend: String,
    /// Why the engine chose its backend.
    pub backend_note: String,
    pub sample_rate: u32,
}

impl UiState {
    /// The track under the cursor, if any.
    pub fn selected(&self) -> Option<&TrackView> {
        self.tracks.get(self.cursor.track)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(name: &str, blocks: &[&str], add_slot: bool) -> TrackView {
        TrackView {
            name: name.into(),
            kind: TrackKind::Midi,
            preset: "Lead".into(),
            waveform: "Square".into(),
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
            blocks: blocks.iter().map(|b| (*b).to_string()).collect(),
            add_slot,
            color_index: 0,
        }
    }

    fn state() -> UiState {
        UiState {
            bpm: 120.0,
            playing: true,
            looping: true,
            transport_position_beats: 5.0,
            cursor: Cursor { track: 0, block: 1 },
            octave: 0,
            tracks: vec![
                track("Midi 1", &["Verse", "Chorus"], false),
                track("Audio 1", &[], true),
            ],
            status: "ready".into(),
            renaming: false,
            backend: "silent".into(),
            backend_note: "silent: no device".into(),
            sample_rate: 44_100,
        }
    }

    #[test]
    fn selected_track_follows_the_cursor() {
        assert_eq!(state().selected().map(|t| t.name.as_str()), Some("Midi 1"));
    }

    #[test]
    fn the_snapshot_crosses_ipc_as_camel_case_json() {
        let json = serde_json::to_value(state()).expect("snapshot should serialize");
        assert_eq!(json["transportPositionBeats"], 5.0);
        assert_eq!(json["tracks"][0]["addSlot"], false);
        assert_eq!(json["tracks"][1]["addSlot"], true);
        assert_eq!(json["tracks"][0]["kind"], "midi");
    }
}
