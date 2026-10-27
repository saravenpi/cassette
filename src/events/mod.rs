//! Typed, bounded messaging between input adapters, the app and the UI.

pub mod bus;
pub mod types;

pub use types::{BeatRange, Event, NoteEdit};
