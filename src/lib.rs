#![forbid(unsafe_code)]
//! zboub69 — a keyboard-only terminal DAW.
//!
//! The crate is split along the boundaries described in `docs/ARCHITECTURE.md`:
//!
//! ```text
//! input ──► events ──► core ◄── ui
//!                        ▲
//!                      models   (shared data only)
//! ```
//!
//! * [`models`] holds the song (tracks, blocks, instruments) as plain data.
//! * [`core`] is the audio engine. It never depends on the terminal.
//! * [`events`] is the typed, bounded message bus between adapters and the app.
//! * [`input`] translates keyboards, MIDI and OSC into [`events::Event`].
//! * [`ui`] is a thin presentation layer over an immutable [`ui::view::UiState`].

pub mod app;
pub mod core;
pub mod events;
pub mod input;
pub mod models;
pub mod ui;
pub mod utils;

pub use crate::core::engine::AudioEngine;
pub use crate::events::Event;
pub use crate::models::Song;
