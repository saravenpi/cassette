//! The typed events that cross module boundaries.
//!
//! Events carry *intent* (`CreateBlock`, `NoteOn`), never presentation data and
//! never raw sample buffers. This is the vocabulary the webview speaks: a
//! Tauri command deserializes one and hands it to the app.

use serde::{Deserialize, Serialize};

use crate::models::EffectKind;

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
    /// Loop between two beats, in either order. A span shorter than the shortest
    /// block clears the region, so the whole song loops again.
    SetLoopRegion {
        start_beats: f32,
        end_beats: f32,
    },
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
    /// Cut a block in two at an absolute beat. The right half keeps playing
    /// the material from where the cut fell.
    SplitBlock {
        track: usize,
        block: usize,
        at_beats: f32,
    },
    /// Copy a block onto the beat right after it.
    DuplicateBlock {
        track: usize,
        block: usize,
    },
    /// Drag a block's left edge: the end stays put and the material under the
    /// new start keeps its place on the timeline.
    TrimBlockStart {
        track: usize,
        block: usize,
        start_beats: f32,
    },
    /// Drag a block's right edge so its material fills the new length, by
    /// changing the playback rate.
    StretchBlock {
        track: usize,
        block: usize,
        length_beats: f32,
    },
    /// Append an empty block to the selected track.
    AddBlock,
    /// Remove the block under the cursor.
    DeleteBlock,
    /// Draw a note into a MIDI block. `start_beats` is relative to the block.
    AddNote {
        track: usize,
        block: usize,
        pitch: u8,
        start_beats: f32,
        length_beats: f32,
        velocity: u8,
    },
    /// Move a note to a new pitch and start, relative to its block.
    MoveNote {
        track: usize,
        block: usize,
        note: usize,
        pitch: u8,
        start_beats: f32,
    },
    /// Drag a note's right edge.
    ResizeNote {
        track: usize,
        block: usize,
        note: usize,
        length_beats: f32,
    },
    /// Change how hard a note is struck.
    SetNoteVelocity {
        track: usize,
        block: usize,
        note: usize,
        velocity: u8,
    },
    /// Remove one note from one block.
    DeleteNote {
        track: usize,
        block: usize,
        note: usize,
    },
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
    /// Arm or disarm recording the played notes into the timeline.
    ToggleRecord,
    /// Turn the metronome click on or off.
    ToggleMetronome,
    /// Bars the metronome counts in before a recording that starts from stop.
    SetCountIn {
        bars: u8,
    },
    /// Grid recorded note starts snap to, in beats; `0` records unquantized.
    SetRecordQuantize {
        grid_beats: f32,
    },
    /// Snap every note start in a MIDI block to a grid, keeping lengths.
    QuantizeBlock {
        track: usize,
        block: usize,
        grid_beats: f32,
    },
    /// Insert an effect into a track's chain at `index`, or at the end.
    AddEffect {
        track: usize,
        kind: EffectKind,
        index: Option<usize>,
    },
    /// Take an effect out of a track's chain.
    RemoveEffect {
        track: usize,
        index: usize,
    },
    /// Move an effect to another place in its chain.
    MoveEffect {
        track: usize,
        from: usize,
        to: usize,
    },
    /// Switch an effect on, or bypass it.
    SetEffectEnabled {
        track: usize,
        index: usize,
        enabled: bool,
    },
    /// Set one parameter of an effect, in the units of its spec.
    SetEffectParam {
        track: usize,
        index: usize,
        param: usize,
        value: f32,
    },
    /// Export the project structure to a JSON file.
    ExportProject {
        path: Option<String>,
    },
    /// Load the project structure from a JSON file.
    LoadProject {
        path: Option<String>,
    },
    /// Step back to the song before the last edit.
    Undo,
    /// Step forward to the song the last undo went back from.
    Redo,
    /// Close the gesture collecting drag events, so the next drag is its own
    /// undo step. The webview sends it when a drag begins.
    Checkpoint,
}
