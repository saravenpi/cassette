use crate::events::types::Event;
use crate::input::keymap::KeyMap;

pub struct InputHandler {
    keymap: KeyMap,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            keymap: KeyMap::new(),
        }
    }

    pub fn handle_key(&self, key: &str) -> Vec<Event> {
        self.keymap.resolve(key).cloned().unwrap_or_default()
    }
}
