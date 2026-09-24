//! The two-pane focus model.

/// Which pane currently owns the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// The session track list: blocks, meters and mixer strip.
    Tracks,
    /// The selected track's instrument.
    Inspector,
}

impl Pane {
    /// Title shown in the pane's border.
    pub const fn title(self) -> &'static str {
        match self {
            Pane::Tracks => "Tracks",
            Pane::Inspector => "Inspector",
        }
    }
}
