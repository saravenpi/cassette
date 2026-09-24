//! The typed events that cross module boundaries.
//!
//! Events carry *intent* (`AddBlock`, `NoteOn`), never presentation data and
//! never raw sample buffers.

/// A single piece of user intent.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Leave the application.
    Quit,
    /// Start playing, or stop if already playing.
    TogglePlay,
    /// Stop and rewind.
    Stop,
    /// Enable or disable looping.
    ToggleLoop,
    /// Show or hide the help overlay.
    ToggleHelp,
    /// Move the track cursor.
    MoveTrack(i32),
    /// Move the block cursor within the selected track.
    MoveBlock(i32),
    /// Append an empty block to the selected track.
    AddBlock,
    /// Remove the block under the cursor.
    DeleteBlock,
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
    /// Change the tempo.
    BpmDelta(f32),
    /// Toggle mute on the selected track.
    ToggleMute,
    /// Toggle solo on the selected track.
    ToggleSolo,
    /// Transpose the play keyboard.
    OctaveDelta(i32),
    /// A note was played on the keyboard.
    NoteOn { pitch: u8, velocity: u8 },
    /// A played note was released.
    NoteOff { pitch: u8 },
    /// A control-surface value arrived over OSC.
    OscControl { address: String, value: f32 },
    /// The terminal was resized.
    Resize(u16, u16),
}
