//! The typed events that cross module boundaries.
//!
//! Events carry *intent* (`ToggleStep`, `NoteOn`), never presentation data and
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
    /// Arm or disarm performance recording.
    ToggleRecord,
    /// Enable or disable pattern looping.
    ToggleLoop,
    /// Show or hide the help overlay.
    ToggleHelp,
    /// Cycle the focused pane.
    CycleFocus,
    /// Move the pattern cursor.
    MoveCursor { dx: i32, dy: i32 },
    /// Toggle the step under the cursor.
    ToggleStep,
    /// Transpose the step under the cursor.
    PitchDelta(i32),
    /// Change the velocity of the step under the cursor.
    VelocityDelta(i32),
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
    /// Advance the selected track's waveform.
    CycleWaveform,
    /// Move the selected track through the instrument presets.
    CyclePreset,
    /// Transpose the play keyboard.
    OctaveDelta(i32),
    /// Change the pattern length.
    LengthDelta(i32),
    /// A note was played on the keyboard.
    NoteOn { pitch: u8, velocity: u8 },
    /// A played note was released.
    NoteOff { pitch: u8 },
    /// A control-surface value arrived over OSC.
    OscControl { address: String, value: f32 },
    /// Bounce the song to a WAV file.
    Export,
    /// The terminal was resized.
    Resize(u16, u16),
}
