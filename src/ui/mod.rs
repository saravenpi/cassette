//! The thin terminal presentation layer.
//!
//! `ui` depends on `core`, `events` and `models`, but never on `input`.
//! [`view::UiState`] is an immutable snapshot of everything worth drawing, which
//! lets alternate renderers (see [`view::View`]) share one source of truth.

pub mod layout;
pub mod renderer;
pub mod view;

pub use layout::Pane;
pub use view::{Cursor, TrackView, UiState, View};
