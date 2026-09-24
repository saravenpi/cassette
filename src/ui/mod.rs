//! The presentation contract.
//!
//! `ui` depends on `core`, `events` and `models`, but never on the webview.
//! [`view::UiState`] is an immutable, serializable snapshot of everything worth
//! drawing, which the Tauri shell hands to the SvelteKit frontend each frame.

pub mod view;

pub use view::{Cursor, TrackView, UiState};
