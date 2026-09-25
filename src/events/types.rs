//! The typed events that cross module boundaries.
//!
//! Events carry *intent* (`CreateBlock`, `NoteOn`), never presentation data and
//! never raw sample buffers. This is the vocabulary the webview speaks: a
//! Tauri command deserializes one and hands it to the app.

use serde::{Deserialize, Serialize};

/// A single piece of user intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    TogglePlay,
    Stop,
    Seek {
        beats: f32,
    },
    ToggleLoop,
    /// Move the track cursor.
    MoveTrack(i32),
    /// Move the block cursor within the selected track.
    MoveBlock(i32),
    /// Put the cursor on a track, without moving the block cursor.
    SelectTrack {
        track: usize,
    },
    /// Put the cursor on one block of one track.
    SelectBlock {
        track: usize,
        block: usize,
    },
    /// Create an empty block of the track's kind on its lane.
    CreateBlock {
        track: usize,
        start_beats: f32,
        length_beats: f32,
    },
    /// Move a block to a new start, possibly onto another track. This is the
    /// drag: the webview sends it as the pointer moves.
    PlaceBlock {
        track: usize,
        block: usize,
        target_track: usize,
        start_beats: f32,
    },
    /// Drag a block's right edge.
    ResizeBlock {
        track: usize,
        block: usize,
        length_beats: f32,
    },
    /// Remove one block from one track.
    DeleteBlockAt {
        track: usize,
        block: usize,
    },
    /// Append an empty block to the selected track.
    AddBlock,
    /// Remove the block under the cursor.
    DeleteBlock,
    /// Drop an audio file onto a track, creating an audio block that plays it.
    ImportSample {
        track: usize,
        path: String,
        start_beats: f32,
    },
    /// Flip the selected track between MIDI and audio.
    CycleTrackKind,
    /// Start, then commit, the block-name prompt.
    RenameBlock,
    /// A character typed into the block-name prompt.
    TextInput(char),
    /// A backspace in the block-name prompt.
    TextBackspace,
    /// Abandon the block-name prompt.
    CancelRename,
    /// Change the selected track's volume.
    VolumeDelta(f32),
    /// Change the selected track's pan.
    PanDelta(f32),
    /// Set a track's whole mixer strip at once, which is what a fader drag is.
    SetTrackMix {
        track: usize,
        volume: f32,
        pan: f32,
        muted: bool,
        solo: bool,
    },
    /// Change the tempo.
    BpmDelta(f32),
    /// Toggle mute on the selected track.
    ToggleMute,
    /// Toggle solo on the selected track.
    ToggleSolo,
    /// Set a track's built-in voice preset by name, as the VST pane does.
    SetPreset {
        track: usize,
        preset: String,
    },
    /// Edit a track's voice: waveform character and envelope.
    SetInstrument {
        track: usize,
        gain: f32,
        octave: i32,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
    },
    /// Transpose the play keyboard.
    OctaveDelta(i32),
    /// A note was played on the keyboard.
    NoteOn {
        pitch: u8,
        velocity: u8,
    },
    /// A played note was released.
    NoteOff {
        pitch: u8,
    },
    /// A control-surface value arrived over OSC.
    OscControl {
        address: String,
        value: f32,
    },
    /// Export the project audio to a WAV file.
    ExportAudio {
        path: Option<String>,
    },
    /// Record the project audio to a file.
    RecordAudio {
        path: Option<String>,
    },
    /// Toggle live recording.
    ToggleRecord,
    /// Export the project structure to a JSON file.
    ExportProject {
        path: Option<String>,
    },
}
