//! Domain data. No behaviour that depends on the terminal or the audio device.

pub mod instrument;
pub mod pattern;
pub mod song;
pub mod track;

pub use instrument::{Instrument, Waveform};
pub use pattern::Step;
pub use song::Song;
pub use track::Track;
