//! Sample slots, written by the control thread and read by the audio thread.
//!
//! Decoding happens on the control thread; the frames then travel down an
//! `rtrb` ring and are copied into a pool that was allocated before the stream
//! started. Nothing here allocates on the audio side, so the red-zone contract
//! holds: [`SamplePool::drain`] only writes into memory that already exists.

use rtrb::Consumer;
use std::sync::Arc;

pub const MAX_SAMPLE_SLOTS: usize = 8;
pub const MAX_SAMPLE_FRAMES: usize = 480_000;
pub const SAMPLE_RING_CAPACITY: usize = MAX_SAMPLE_FRAMES + 16;
pub const NO_SLOT: u8 = u8::MAX;

#[derive(Clone, Debug, PartialEq)]
pub enum SampleMsg {
    Begin {
        slot: u8,
        frames: u32,
        rate: f32,
    },
    Frame(f32),
    End {
        slot: u8,
    },
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

#[derive(Clone, Copy, Debug)]
struct Loading {
    slot: u8,
    total: u32,
    written: u32,
}

pub struct SamplePool {
    slots: [Option<Arc<[f32]>>; MAX_SAMPLE_SLOTS],
    data: Vec<f32>,
    lengths: [u32; MAX_SAMPLE_SLOTS],
    rates: [f32; MAX_SAMPLE_SLOTS],
    loading: Option<Loading>,
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
            data: vec![0.0; MAX_SAMPLE_SLOTS * MAX_SAMPLE_FRAMES],
            lengths: [0; MAX_SAMPLE_SLOTS],
            rates: [1.0; MAX_SAMPLE_SLOTS],
            loading: None,
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
            SampleMsg::Begin { slot, frames, rate } => {
                let slot = slot as usize;
                if slot < MAX_SAMPLE_SLOTS {
                    self.slots[slot] = None;
                    self.loading = Some(Loading {
                        slot: slot as u8,
                        total: frames.min(MAX_SAMPLE_FRAMES as u32),
                        written: 0,
                    });
                    self.lengths[slot] = 0;
                    self.rates[slot] = if rate > 0.0 { rate } else { 1.0 };
                }
            }
            SampleMsg::Frame(value) => {
                let Some(loading) = self.loading.as_mut() else {
                    return;
                };
                if loading.written >= loading.total {
                    return;
                }
                let at = loading.slot as usize * MAX_SAMPLE_FRAMES + loading.written as usize;
                if let Some(cell) = self.data.get_mut(at) {
                    *cell = value;
                    loading.written += 1;
                }
            }
            SampleMsg::End { .. } => {
                if let Some(loading) = self.loading.take()
                    && let Some(length) = self.lengths.get_mut(loading.slot as usize)
                {
                    *length = loading.written;
                }
            }
            SampleMsg::Free { slot } => {
                let slot = slot as usize;
                if slot < MAX_SAMPLE_SLOTS {
                    self.slots[slot] = None;
                    self.lengths[slot] = 0;
                }
            }
        }
    }

    pub fn frames(&self, slot: u8) -> &[f32] {
        let slot = slot as usize;
        if slot >= MAX_SAMPLE_SLOTS {
            return &[];
        }
        if let Some(frames) = &self.slots[slot] {
            return frames;
        }
        let length = self.lengths[slot] as usize;
        let start = slot * MAX_SAMPLE_FRAMES;
        self.data.get(start..start + length).unwrap_or(&[])
    }

    pub fn rate(&self, slot: u8) -> f32 {
        let slot = slot as usize;
        if slot >= MAX_SAMPLE_SLOTS {
            return 0.0;
        }
        self.rates[slot]
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
        let (mut producer, mut consumer) = RingBuffer::<SampleMsg>::new(64);
        for message in messages {
            producer
                .push(message.clone())
                .expect("ring should have room");
        }
        pool.drain(&mut consumer);
    }

    #[test]
    fn an_arc_load_becomes_readable() {
        let mut pool = SamplePool::new();
        let frames: Arc<[f32]> = Arc::from(vec![0.1, 0.2, 0.3].into_boxed_slice());
        load(
            &mut pool,
            &[SampleMsg::Load {
                slot: 0,
                frames: frames.clone(),
                rate: 44_100.0,
            }],
        );
        assert!(pool.is_ready(0));
        assert_eq!(pool.frames(0), &[0.1, 0.2, 0.3]);
        assert_eq!(pool.rate(0), 44_100.0);
    }

    #[test]
    fn a_completed_load_becomes_readable() {
        let mut pool = SamplePool::new();
        load(
            &mut pool,
            &[
                SampleMsg::Begin {
                    slot: 1,
                    frames: 3,
                    rate: 44_100.0,
                },
                SampleMsg::Frame(0.25),
                SampleMsg::Frame(-0.5),
                SampleMsg::Frame(0.75),
                SampleMsg::End { slot: 1 },
            ],
        );
        assert!(pool.is_ready(1));
        assert_eq!(pool.frames(1), &[0.25, -0.5, 0.75]);
        assert_eq!(pool.rate(1), 44_100.0);
        assert!(!pool.is_ready(0), "another slot is untouched");
    }

    #[test]
    fn a_load_in_progress_is_not_playable_yet() {
        let mut pool = SamplePool::new();
        load(
            &mut pool,
            &[
                SampleMsg::Begin {
                    slot: 0,
                    frames: 4,
                    rate: 48_000.0,
                },
                SampleMsg::Frame(1.0),
            ],
        );
        assert!(
            !pool.is_ready(0),
            "half a sample must not be played as if it were whole"
        );
    }

    #[test]
    fn more_frames_than_promised_are_dropped() {
        let mut pool = SamplePool::new();
        load(
            &mut pool,
            &[
                SampleMsg::Begin {
                    slot: 2,
                    frames: 1,
                    rate: 44_100.0,
                },
                SampleMsg::Frame(0.5),
                SampleMsg::Frame(0.9),
                SampleMsg::End { slot: 2 },
            ],
        );
        assert_eq!(pool.frames(2), &[0.5]);
    }

    #[test]
    fn freeing_a_slot_empties_it() {
        let mut pool = SamplePool::new();
        load(
            &mut pool,
            &[
                SampleMsg::Begin {
                    slot: 3,
                    frames: 1,
                    rate: 44_100.0,
                },
                SampleMsg::Frame(0.5),
                SampleMsg::End { slot: 3 },
                SampleMsg::Free { slot: 3 },
            ],
        );
        assert!(!pool.is_ready(3));
    }

    #[test]
    fn an_end_with_a_stray_slot_is_ignored() {
        let mut pool = SamplePool::new();
        load(
            &mut pool,
            &[
                SampleMsg::Begin {
                    slot: 1,
                    frames: 2,
                    rate: 44_100.0,
                },
                SampleMsg::Frame(0.5),
                SampleMsg::End { slot: 250 },
            ],
        );
        assert_eq!(pool.frames(1), &[0.5]);
        load(&mut pool, &[SampleMsg::End { slot: 250 }]);
    }

    #[test]
    fn a_frame_with_no_load_is_ignored() {
        let mut pool = SamplePool::new();
        load(&mut pool, &[SampleMsg::Frame(1.0)]);
        assert!(!pool.is_ready(0));
    }

    #[test]
    fn slots_do_not_overlap() {
        let mut pool = SamplePool::new();
        for slot in 0..2u8 {
            load(
                &mut pool,
                &[
                    SampleMsg::Begin {
                        slot,
                        frames: 2,
                        rate: 44_100.0,
                    },
                    SampleMsg::Frame(slot as f32),
                    SampleMsg::Frame(slot as f32 + 0.5),
                    SampleMsg::End { slot },
                ],
            );
        }
        assert_eq!(pool.frames(0), &[0.0, 0.5]);
        assert_eq!(pool.frames(1), &[1.0, 1.5]);
    }

    #[test]
    fn the_pool_holds_a_full_slot() {
        let mut pool = SamplePool::new();
        let (mut producer, mut consumer) = RingBuffer::<SampleMsg>::new(MAX_SAMPLE_FRAMES + 2);
        producer
            .push(SampleMsg::Begin {
                slot: 0,
                frames: MAX_SAMPLE_FRAMES as u32,
                rate: 44_100.0,
            })
            .expect("room for the header");
        for _ in 0..MAX_SAMPLE_FRAMES {
            producer
                .push(SampleMsg::Frame(0.1))
                .expect("room for frames");
        }
        producer
            .push(SampleMsg::End { slot: 0 })
            .expect("room for the tail");
        pool.drain(&mut consumer);
        assert_eq!(pool.frames(0).len(), MAX_SAMPLE_FRAMES);
    }
}
