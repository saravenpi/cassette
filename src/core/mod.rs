//! The audio engine. Nothing in this module knows about the terminal.
//!
//! # Real-time contract (the "red zone")
//!
//! [`render::RenderCore::process`] runs inside the audio callback. It must never
//! allocate, take a lock, perform I/O, or panic. Everything it touches is
//! pre-allocated at construction time and driven by [`render::RtCommand`]s
//! pushed through a lock-free `rtrb` ring buffer.

pub mod audio;
pub mod engine;
pub mod meter;
pub mod render;
pub mod synth;

/// Hard ceiling on tracks. Keeps every audio-side array fixed-size.
pub const MAX_TRACKS: usize = 16;
/// Hard ceiling on blocks per track, mirroring the UI-side list.
pub const MAX_BLOCKS: usize = 16;
/// Voices per track (polyphony).
pub const MAX_VOICES: usize = 8;
/// Sample rate used when no device suggests one.
pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;
/// Capacity of the UI -> audio command ring buffer.
pub const RT_COMMAND_CAPACITY: usize = 4096;
