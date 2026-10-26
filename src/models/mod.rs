//! Domain data. No behaviour that depends on the terminal or the audio device.

pub mod effect;
pub mod instrument;
pub mod note;
pub mod song;
pub mod track;

pub use effect::{Effect, EffectChain, EffectKind, MAX_EFFECTS, MAX_PARAMS, ParamSpec, ParamUnit};
pub use instrument::{Instrument, Waveform};
pub use note::{MAX_NOTES_PER_BLOCK, Note};
pub use song::{LoopRegion, Song};
pub use track::{Block, MAX_LENGTH_BEATS, MAX_START_BEATS, Track, TrackKind};
