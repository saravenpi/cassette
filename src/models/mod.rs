//! Domain data. No behaviour that depends on the terminal or the audio device.

pub mod instrument;
pub mod song;
pub mod track;

pub use instrument::{Instrument, Waveform};
pub use song::Song;
pub use track::{Block, Track, TrackKind};
