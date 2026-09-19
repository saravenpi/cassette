//! A bounded, fan-out event bus.
//!
//! Every subscriber gets its own bounded channel, so a slow consumer cannot
//! stall a producer: sends that would exceed the bound fail fast and are
//! reported rather than blocking.

use std::sync::Arc;

use crossbeam::channel::{Receiver, Sender, TrySendError, bounded};

use crate::events::types::Event;

/// Capacity of each subscriber's channel.
pub const CHANNEL_CAPACITY: usize = 1024;

/// A small pub/sub bus with fixed-capacity channels.
pub struct EventBus {
    subscribers: Vec<Arc<Sender<Event>>>,
}

impl EventBus {
    /// Create a bus with no subscribers.
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Register a subscriber and return its receiver.
    pub fn subscribe(&mut self) -> Receiver<Event> {
        let (sender, receiver) = bounded(CHANNEL_CAPACITY);
        self.subscribers.push(Arc::new(sender));
        receiver
    }

    /// Number of live subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    /// Broadcast an event. Returns the number of subscribers that received it.
    pub fn send(&self, event: Event) -> usize {
        let mut delivered = 0;
        for sender in &self.subscribers {
            match sender.try_send(event.clone()) {
                Ok(()) => delivered += 1,
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
        delivered
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventBus {
    /// Cloning shares the existing subscribers (it does not duplicate them).
    fn clone(&self) -> Self {
        Self {
            subscribers: self.subscribers.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_fan_out_to_every_subscriber() {
        let mut bus = EventBus::new();
        let a = bus.subscribe();
        let b = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        assert_eq!(bus.send(Event::TogglePlay), 2);
        assert_eq!(a.try_recv().unwrap(), Event::TogglePlay);
        assert_eq!(b.try_recv().unwrap(), Event::TogglePlay);
    }

    #[test]
    fn dropped_subscribers_are_skipped() {
        let mut bus = EventBus::new();
        let receiver = bus.subscribe();
        assert_eq!(bus.send(Event::Quit), 1);
        drop(receiver);
        assert_eq!(bus.send(Event::Quit), 0);
    }

    #[test]
    fn a_full_channel_does_not_block_the_sender() {
        let mut bus = EventBus::new();
        let _receiver = bus.subscribe();
        for _ in 0..CHANNEL_CAPACITY + 5 {
            bus.send(Event::Resize(80, 24));
        }
    }
}
