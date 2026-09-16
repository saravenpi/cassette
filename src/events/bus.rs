use crate::events::types::Event;
use crossbeam::channel::{bounded, Receiver, Sender};
use std::sync::Arc;

pub struct EventBus {
    subscribers: Vec<Arc<Sender<Event>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    pub fn subscribe(&mut self) -> Receiver<Event> {
        let (tx, rx) = bounded(1000);
        self.subscribers.push(Arc::new(tx));
        rx
    }

    pub fn send(&self, event: Event) {
        for (i, tx) in self.subscribers.iter().enumerate() {
            if tx.send(event.clone()).is_err() {
                eprintln!("[bus] subscriber {i} disconnected");
            }
        }
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: self.subscribers.clone(),
        }
    }
}
