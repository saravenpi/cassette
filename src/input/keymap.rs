use crate::events::types::Event;
use std::collections::HashMap;

pub type Key = String;

pub struct KeyMap {
    bindings: HashMap<Key, Vec<Event>>,
}

impl KeyMap {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, key: Key, events: Vec<Event>) {
        self.bindings.insert(key, events);
    }

    pub fn resolve(&self, key: &str) -> Option<&Vec<Event>> {
        self.bindings.get(key)
    }
}
