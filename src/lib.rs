#![forbid(unsafe_code)]
//! zboub69 — a session DAW with a Rust audio engine and a SvelteKit frontend.
//!
//! The crate is the engine and the app logic. It never depends on a frontend:
//! the Tauri shell in `app/src-tauri/` drives it and the webview only draws.
//!
//! ```text
//! input ──► events ──► core ◄── ui
//!                        ▲
//!                      models   (shared data only)
//! ```
//!
//! * [`models`] holds the song (tracks, blocks, instruments) as plain data.
//! * [`core`] is the audio engine. Real-time, allocation-free, frontend-agnostic.
//! * [`events`] is the typed, bounded message bus between adapters and the app.
//! * [`input`] translates MIDI and OSC into [`events::Event`].
//! * [`ui`] is the serializable [`ui::view::UiState`] snapshot the webview draws.

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
