//! Sample slots, written by the control thread and read by the audio thread.
//!
//! Decoding happens on the control thread; the decoded frames then travel down
//! an `rtrb` ring as an `Arc` slice the sender keeps a clone of. The audio side
//! only swaps handles, so it never allocates and never frees a buffer, and the
//! red-zone contract holds.

use rtrb::Consumer;
use std::sync::Arc;

pub const MAX_SAMPLE_SLOTS: usize = 8;
/// Room on the ring for every slot to be freed and loaded several times over
/// between two callbacks.
pub const SAMPLE_RING_CAPACITY: usize = MAX_SAMPLE_SLOTS * 8;
pub const NO_SLOT: u8 = u8::MAX;

#[derive(Clone, Debug, PartialEq)]
pub enum SampleMsg {
    Free {
        slot: u8,
    },
    /// Hand over a decoded sample. The sender must keep its own clone of
    /// `frames` for as long as the slot is in use, so the audio thread never
    /// drops the last reference.
    Load {
        slot: u8,
        frames: Arc<[f32]>,
        rate: f32,
    },
}

pub struct SamplePool {
    slots: [Option<Arc<[f32]>>; MAX_SAMPLE_SLOTS],
    rates: [f32; MAX_SAMPLE_SLOTS],
}

impl Default for SamplePool {
    fn default() -> Self {
        Self::new()
    }
}

impl SamplePool {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            rates: [1.0; MAX_SAMPLE_SLOTS],
        }
    }

    pub fn drain(&mut self, consumer: &mut Consumer<SampleMsg>) {
        while let Ok(message) = consumer.pop() {
            self.apply(message);
        }
    }

    pub fn push(&mut self, message: SampleMsg) {
        self.apply(message);
    }

    fn apply(&mut self, message: SampleMsg) {
        match message {
            SampleMsg::Load { slot, frames, rate } => {
                let slot = slot as usize;
                if slot < MAX_SAMPLE_SLOTS {
                    self.slots[slot] = Some(frames);
                    self.rates[slot] = if rate > 0.0 { rate } else { 1.0 };
                }
            }
            SampleMsg::Free { slot } => {
                if let Some(held) = self.slots.get_mut(slot as usize) {
                    *held = None;
                }
            }
        }
    }

    pub fn frames(&self, slot: u8) -> &[f32] {
        self.slots
            .get(slot as usize)
            .and_then(Option::as_deref)
            .unwrap_or(&[])
    }

    pub fn rate(&self, slot: u8) -> f32 {
        self.rates.get(slot as usize).copied().unwrap_or(0.0)
    }

    pub fn is_ready(&self, slot: u8) -> bool {
        !self.frames(slot).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    fn load(pool: &mut SamplePool, messages: &[SampleMsg]) {
        let (mut producer, mut consumer) = RingBuffer::<SampleMsg>::new(SAMPLE_RING_CAPACITY);
        for message in messages {
            producer
                .push(message.clone())
                .expect("ring should have room");
        }
        pool.drain(&mut consumer);
    }

    fn sample(slot: u8, frames: &[f32]) -> SampleMsg {
        SampleMsg::Load {
            slot,
            frames: Arc::from(frames),
            rate: 44_100.0,
        }
    }

    #[test]
    fn an_arc_load_becomes_readable() {
        let mut pool = SamplePool::new();
        load(&mut pool, &[sample(0, &[0.1, 0.2, 0.3])]);
        assert!(pool.is_ready(0));
        assert_eq!(pool.frames(0), &[0.1, 0.2, 0.3]);
        assert_eq!(pool.rate(0), 44_100.0);
        assert!(!pool.is_ready(1), "another slot is untouched");
    }

    #[test]
    fn freeing_a_slot_empties_it() {
        let mut pool = SamplePool::new();
        load(&mut pool, &[sample(3, &[0.5]), SampleMsg::Free { slot: 3 }]);
        assert!(!pool.is_ready(3));
    }

    #[test]
    fn stray_slots_are_ignored() {
        let mut pool = SamplePool::new();
        load(
            &mut pool,
            &[sample(250, &[0.5]), SampleMsg::Free { slot: 250 }],
        );
        assert!(pool.frames(250).is_empty());
        assert_eq!(pool.rate(250), 0.0);
    }

    #[test]
    fn a_rate_that_is_not_positive_falls_back_to_one() {
        let mut pool = SamplePool::new();
        load(
            &mut pool,
            &[SampleMsg::Load {
                slot: 1,
                frames: Arc::from(&[0.5][..]),
                rate: 0.0,
            }],
        );
        assert_eq!(pool.rate(1), 1.0);
    }

    #[test]
    fn slots_do_not_overlap() {
        let mut pool = SamplePool::new();
        load(&mut pool, &[sample(0, &[0.0, 0.5]), sample(1, &[1.0, 1.5])]);
        assert_eq!(pool.frames(0), &[0.0, 0.5]);
        assert_eq!(pool.frames(1), &[1.0, 1.5]);
    }
}
