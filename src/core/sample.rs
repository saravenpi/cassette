//! Sample slots, written by the control thread and read by the audio thread.
//!
//! Decoding happens on the control thread; the decoded frames then travel down
//! an `rtrb` ring as an `Arc` slice the sender keeps a clone of. The audio side
//! only swaps handles, so it never allocates and never frees a buffer, and the
//! red-zone contract holds.
//!
//! The slot table is unbounded. When the control thread runs out of room it
//! builds a larger [`SampleTable`] and sends it as [`SampleMsg::Grow`]. The
//! audio side moves every slot into it, swaps it in and sends the old table
//! back on a return ring, so the control thread does the freeing.

use rtrb::{Consumer, Producer, PushError};
use std::sync::Arc;

/// Index of a sample slot.
pub type Slot = u32;

/// Slots in a freshly built pool, before any growth.
pub const INITIAL_SAMPLE_SLOTS: usize = 8;
/// Room on the ring for a burst of loads, frees and growths between two
/// callbacks.
pub const SAMPLE_RING_CAPACITY: usize = 256;
/// Room on the return ring. Each growth at least doubles the table, so a `u32`
/// slot space allows fewer growths than this and the ring can never fill.
pub const SAMPLE_RETURN_CAPACITY: usize = 32;
pub const NO_SLOT: Slot = Slot::MAX;

/// The audio side's slot storage: one entry and one pitch ratio per slot.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleTable {
    slots: Vec<Option<Arc<[f32]>>>,
    ratios: Vec<f32>,
}

impl SampleTable {
    /// An empty table with `capacity` slots. Allocates, so it is built on the
    /// control thread.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            ratios: vec![1.0; capacity],
        }
    }

    /// Number of slots the table holds.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SampleMsg {
    Free {
        slot: Slot,
    },
    /// Hand over a decoded sample. The sender must keep its own clone of
    /// `frames` for as long as the slot is in use, so the audio thread never
    /// drops the last reference.
    Load {
        slot: Slot,
        frames: Arc<[f32]>,
        rate: f32,
    },
    /// Replace the slot table with a larger, empty one. The audio side moves
    /// the current contents across and returns the old table.
    Grow {
        table: Box<SampleTable>,
    },
}

/// Control-side bookkeeping: which slots are taken and how big the audio
/// side's table is.
#[derive(Debug)]
pub struct SlotAllocator {
    capacity: usize,
    next: Slot,
    free: Vec<Slot>,
}

impl Default for SlotAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl SlotAllocator {
    pub fn new() -> Self {
        Self {
            capacity: INITIAL_SAMPLE_SLOTS,
            next: 0,
            free: Vec::new(),
        }
    }

    /// Take the lowest recycled slot, or the next fresh one. `None` only when
    /// the whole `u32` space is used.
    pub fn alloc(&mut self) -> Option<Slot> {
        if let Some(slot) = self.free.pop() {
            return Some(slot);
        }
        let slot = self.next;
        if slot == NO_SLOT {
            return None;
        }
        self.next += 1;
        Some(slot)
    }

    /// Give a slot back for reuse.
    pub fn release(&mut self, slot: Slot) {
        if slot < self.next && !self.free.contains(&slot) {
            self.free.push(slot);
            self.free.sort_unstable_by(|a, b| b.cmp(a));
        }
    }

    /// Forget every allocation. The audio table keeps its size.
    pub fn clear(&mut self) {
        self.next = 0;
        self.free.clear();
    }

    /// A larger table when `slot` does not fit the current one. Call
    /// [`SlotAllocator::commit_growth`] once it is on the ring.
    pub fn growth_for(&self, slot: Slot) -> Option<Box<SampleTable>> {
        let needed = slot as usize + 1;
        (needed > self.capacity)
            .then(|| Box::new(SampleTable::with_capacity(needed.next_power_of_two())))
    }

    /// Record that a table of `capacity` slots was sent.
    pub fn commit_growth(&mut self, capacity: usize) {
        self.capacity = capacity;
    }
}

pub struct SamplePool {
    table: Box<SampleTable>,
    engine_rate: f32,
    returns: Option<Producer<Box<SampleTable>>>,
}

impl SamplePool {
    /// A pool of [`INITIAL_SAMPLE_SLOTS`] slots for an engine running at
    /// `engine_rate`.
    pub fn new(engine_rate: f32) -> Self {
        Self {
            table: Box::new(SampleTable::with_capacity(INITIAL_SAMPLE_SLOTS)),
            engine_rate: engine_rate.max(1.0),
            returns: None,
        }
    }

    /// Where replaced tables go, so the control thread frees them.
    pub fn set_return_ring(&mut self, producer: Producer<Box<SampleTable>>) {
        self.returns = Some(producer);
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
                if slot < self.table.slots.len() {
                    self.table.slots[slot] = Some(frames);
                    self.table.ratios[slot] = if rate > 0.0 {
                        rate / self.engine_rate
                    } else {
                        1.0
                    };
                }
            }
            SampleMsg::Free { slot } => {
                if let Some(held) = self.table.slots.get_mut(slot as usize) {
                    *held = None;
                }
            }
            SampleMsg::Grow { table } => self.grow(table),
        }
    }

    fn grow(&mut self, mut table: Box<SampleTable>) {
        if table.slots.len() <= self.table.slots.len() {
            self.retire(table);
            return;
        }
        for index in 0..self.table.slots.len() {
            std::mem::swap(&mut table.slots[index], &mut self.table.slots[index]);
            std::mem::swap(&mut table.ratios[index], &mut self.table.ratios[index]);
        }
        std::mem::swap(&mut self.table, &mut table);
        self.retire(table);
    }

    fn retire(&mut self, table: Box<SampleTable>) {
        let Some(returns) = self.returns.as_mut() else {
            return;
        };
        if let Err(PushError::Full(table)) = returns.push(table) {
            std::mem::forget(table);
        }
    }

    /// Slots the table currently holds.
    pub fn capacity(&self) -> usize {
        self.table.slots.len()
    }

    pub fn frames(&self, slot: Slot) -> &[f32] {
        self.table
            .slots
            .get(slot as usize)
            .and_then(Option::as_deref)
            .unwrap_or(&[])
    }

    /// Ratio of the slot's file rate to the engine's, `0.0` for a slot outside
    /// the table.
    pub fn ratio(&self, slot: Slot) -> f32 {
        self.table.ratios.get(slot as usize).copied().unwrap_or(0.0)
    }

    pub fn is_ready(&self, slot: Slot) -> bool {
        !self.frames(slot).is_empty()
    }

    /// Slots currently holding a finished load.
    pub fn ready_count(&self) -> usize {
        self.table
            .slots
            .iter()
            .filter(|held| held.as_deref().is_some_and(|frames| !frames.is_empty()))
            .count()
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

    fn sample(slot: Slot, frames: &[f32]) -> SampleMsg {
        SampleMsg::Load {
            slot,
            frames: Arc::from(frames),
            rate: 44_100.0,
        }
    }

    #[test]
    fn an_arc_load_becomes_readable() {
        let mut pool = SamplePool::new(44_100.0);
        load(&mut pool, &[sample(0, &[0.1, 0.2, 0.3])]);
        assert!(pool.is_ready(0));
        assert_eq!(pool.frames(0), &[0.1, 0.2, 0.3]);
        assert_eq!(pool.ratio(0), 1.0);
        assert!(!pool.is_ready(1), "another slot is untouched");
    }

    #[test]
    fn freeing_a_slot_empties_it() {
        let mut pool = SamplePool::new(44_100.0);
        load(&mut pool, &[sample(3, &[0.5]), SampleMsg::Free { slot: 3 }]);
        assert!(!pool.is_ready(3));
    }

    #[test]
    fn stray_slots_are_ignored() {
        let mut pool = SamplePool::new(44_100.0);
        load(
            &mut pool,
            &[sample(250, &[0.5]), SampleMsg::Free { slot: 250 }],
        );
        assert!(pool.frames(250).is_empty());
        assert_eq!(pool.ratio(250), 0.0);
    }

    #[test]
    fn a_rate_that_is_not_positive_falls_back_to_one() {
        let mut pool = SamplePool::new(44_100.0);
        load(
            &mut pool,
            &[SampleMsg::Load {
                slot: 1,
                frames: Arc::from(&[0.5][..]),
                rate: 0.0,
            }],
        );
        assert_eq!(pool.ratio(1), 1.0);
    }

    #[test]
    fn slots_do_not_overlap() {
        let mut pool = SamplePool::new(44_100.0);
        load(&mut pool, &[sample(0, &[0.0, 0.5]), sample(1, &[1.0, 1.5])]);
        assert_eq!(pool.frames(0), &[0.0, 0.5]);
        assert_eq!(pool.frames(1), &[1.0, 1.5]);
    }

    #[test]
    fn growing_keeps_loaded_slots_and_returns_the_old_table() {
        let (producer, mut returns) = RingBuffer::<Box<SampleTable>>::new(4);
        let mut pool = SamplePool::new(44_100.0);
        pool.set_return_ring(producer);
        load(&mut pool, &[sample(2, &[0.25, 0.5])]);
        pool.push(SampleMsg::Grow {
            table: Box::new(SampleTable::with_capacity(32)),
        });
        assert_eq!(pool.capacity(), 32);
        assert_eq!(pool.frames(2), &[0.25, 0.5]);
        load(&mut pool, &[sample(20, &[1.0])]);
        assert_eq!(pool.frames(20), &[1.0]);
        let old = returns.pop().expect("the old table comes back");
        assert_eq!(old.capacity(), INITIAL_SAMPLE_SLOTS);
        assert!(returns.pop().is_err());
    }

    #[test]
    fn a_smaller_table_is_handed_back_untouched() {
        let (producer, mut returns) = RingBuffer::<Box<SampleTable>>::new(4);
        let mut pool = SamplePool::new(44_100.0);
        pool.set_return_ring(producer);
        pool.push(SampleMsg::Grow {
            table: Box::new(SampleTable::with_capacity(2)),
        });
        assert_eq!(pool.capacity(), INITIAL_SAMPLE_SLOTS);
        assert!(returns.pop().is_ok());
    }

    #[test]
    fn the_allocator_recycles_the_lowest_slot_and_grows_by_doubling() {
        let mut alloc = SlotAllocator::new();
        let slots: Vec<Slot> = (0..9).map(|_| alloc.alloc().unwrap()).collect();
        assert_eq!(slots, (0..9).collect::<Vec<_>>());
        let table = alloc.growth_for(8).expect("slot 8 needs room");
        assert_eq!(table.capacity(), 16);
        alloc.commit_growth(table.capacity());
        assert!(alloc.growth_for(15).is_none());
        alloc.release(5);
        alloc.release(2);
        assert_eq!(alloc.alloc(), Some(2));
        assert_eq!(alloc.alloc(), Some(5));
        assert_eq!(alloc.alloc(), Some(9));
    }

    #[test]
    fn more_than_eight_samples_load_through_growth() {
        let mut alloc = SlotAllocator::new();
        let mut pool = SamplePool::new(44_100.0);
        for index in 0..40 {
            let slot = alloc.alloc().unwrap();
            if let Some(table) = alloc.growth_for(slot) {
                alloc.commit_growth(table.capacity());
                pool.push(SampleMsg::Grow { table });
            }
            pool.push(sample(slot, &[index as f32 + 1.0]));
        }
        assert_eq!(pool.ready_count(), 40);
        assert_eq!(pool.frames(39), &[40.0]);
        assert_eq!(pool.frames(0), &[1.0]);
    }
}
