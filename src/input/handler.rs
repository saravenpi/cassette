//! Terminal input handler.
//!
//! The only place that reads raw key codes. It never renders and never touches
//! the audio engine; it just turns the terminal into [`Event`]s.

use std::time::Duration;

use crossterm::event::{self, Event as TerminalEvent};

use crate::events::Event;
use crate::input::keymap::Keymap;

/// Polls the terminal and maps key presses through a [`Keymap`].
pub struct InputHandler {
    keymap: Keymap,
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl InputHandler {
    /// Create a handler with the default key bindings.
    pub fn new() -> Self {
        Self {
            keymap: Keymap::default(),
        }
    }

    /// Access the keymap (e.g. to change the play octave).
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    /// Read the current play octave.
    pub fn octave(&self) -> i32 {
        self.keymap.octave()
    }

    /// Wait up to `timeout` for input, then drain anything else that is ready.
    pub fn poll(&mut self, timeout: Duration) -> Vec<Event> {
        let mut events = Vec::new();
        if !event::poll(timeout).unwrap_or(false) {
            return events;
        }
        loop {
            match event::read() {
                Ok(TerminalEvent::Key(key)) => {
                    if let Some(event) = self.keymap.map(key) {
                        events.push(event);
                    }
                }
                Ok(TerminalEvent::Resize(width, height)) => {
                    events.push(Event::Resize(width, height));
                }
                Ok(_) => {}
                Err(_) => break,
            }
            if !event::poll(Duration::ZERO).unwrap_or(false) {
                break;
            }
        }
        events
    }
}
