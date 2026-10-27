//! The snapshot the webview draws.
//!
//! [`UiState`] is built once per frame from the song and the audio engine's
//! shared state. It holds no references into `core`, so it crosses the Tauri IPC
//! boundary as plain serializable data.

use serde::{Deserialize, Serialize};

use crate::models::{EffectKind, LoopRegion, Note, ParamUnit, TrackKind};

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
    /// were drawn so an index names a note. Empty when the webview already holds
    /// this track's `notes_revision`.
    pub notes: Vec<Note>,
    /// Seconds into an audio block's file where it starts reading.
    pub offset_seconds: f32,
    /// Playback speed of an audio block's material.
    pub rate: f32,
}

/// One effect in a track's chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectView {
    /// The effect's stable id within its track.
    pub id: u32,
    pub kind: EffectKind,
    pub name: String,
    pub enabled: bool,
    /// One value per entry of the kind's spec, in its units.
    pub params: Vec<f32>,
    /// Track driving the detector, for a kind that takes a sidechain.
    pub sidechain: Option<usize>,
}

/// One parameter of an effect kind, for drawing its dial.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamSpecView {
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: ParamUnit,
    pub log: bool,
    pub choices: Vec<String>,
}

/// One effect kind the engine builds in, for the effect browser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectSpecView {
    pub kind: EffectKind,
    pub name: String,
    pub takes_sidechain: bool,
    pub params: Vec<ParamSpecView>,
}

impl EffectSpecView {
    /// Every built-in kind, in browser order.
    pub fn catalog() -> Vec<EffectSpecView> {
        EffectKind::ALL
            .iter()
            .map(|kind| EffectSpecView {
                kind: *kind,
                name: kind.name().to_string(),
                takes_sidechain: kind.takes_sidechain(),
                params: kind
                    .spec()
                    .iter()
                    .map(|spec| ParamSpecView {
                        name: spec.name.to_string(),
                        min: spec.min,
                        max: spec.max,
                        default: spec.default,
                        unit: spec.unit,
                        log: spec.log,
                        choices: spec.choices.iter().map(|c| c.to_string()).collect(),
                    })
                    .collect(),
            })
            .collect()
    }
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
    /// Changes whenever the notes of any block on the track may have changed.
    pub notes_revision: u64,
    /// Insert effects, in signal order.
    pub effects: Vec<EffectView>,
    /// Whether the instrument sounds; a bypassed one is silent.
    pub instrument_enabled: bool,
    pub cutoff: f32,
    pub resonance: f32,
    pub detune: f32,
    pub sub: f32,
}

/// What the clipboard holds, so the interface can offer Paste.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardView {
    /// Tracks the copied range spans.
    pub tracks: usize,
    pub length_beats: f32,
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
    /// Whether played notes are being written into the timeline.
    pub recording: bool,
    /// Whether the metronome clicks while the transport plays.
    pub metronome: bool,
    /// Bars counted in before a recording that starts from stop.
    pub count_in_bars: u8,
    /// Beats of count-in still to go, or `0` when none is running.
    pub count_in_beats_left: f32,
    /// Grid recorded notes snap to, in beats; `0` when off.
    pub record_quantize_beats: f32,
    pub looping: bool,
    /// Where the transport loops, or `None` for the whole song.
    pub loop_region: Option<LoopRegion>,
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
    /// The built-in effects and their parameters.
    pub effect_catalog: Vec<EffectSpecView>,
    pub tracks: Vec<TrackView>,
    pub status: String,
    /// Whether the block-name prompt is open, so the webview routes typing to it.
    pub renaming: bool,
    /// Whether an undo has a step to go back to.
    pub can_undo: bool,
    /// Whether a redo has a step to go forward to.
    pub can_redo: bool,
    pub backend: String,
    /// Why the engine chose its backend.
    pub backend_note: String,
    pub sample_rate: u32,
    /// Where the project was last saved or loaded from, or `None` when it has
    /// never been saved.
    pub project_path: Option<String>,
    /// Whether the song changed since it was last saved or loaded.
    pub project_dirty: bool,
    pub clipboard: Option<ClipboardView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_progress: Option<f32>,
}

impl UiState {
    /// Drop the notes of every track whose revision the webview already holds,
    /// so a frame carries notes only when they changed. `known[i]` is the
    /// revision the webview has for track `i`.
    pub fn without_known_notes(mut self, known: &[u64]) -> Self {
        for (track, revision) in self.tracks.iter_mut().zip(known) {
            if track.notes_revision == *revision {
                for block in &mut track.blocks {
                    block.notes = Vec::new();
                }
            }
        }
        self
    }
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
            offset_seconds: 0.0,
            rate: 1.0,
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
            notes_revision: 0,
            effects: Vec::new(),
            instrument_enabled: true,
            cutoff: 20_000.0,
            resonance: 0.0,
            detune: 0.0,
            sub: 0.0,
        }
    }

    fn state() -> UiState {
        UiState {
            bpm: 120.0,
            playing: true,
            recording: false,
            metronome: false,
            count_in_bars: 0,
            count_in_beats_left: 0.0,
            record_quantize_beats: 0.0,
            looping: true,
            loop_region: None,
            transport_position_beats: 5.0,
            length_beats: 32.0,
            bars: 8,
            beats_per_bar: 4,
            cursor: Cursor { track: 0, block: 1 },
            octave: 0,
            can_undo: false,
            can_redo: false,
            effect_catalog: Vec::new(),
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
            project_path: None,
            project_dirty: false,
            clipboard: None,
            export_progress: None,
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
