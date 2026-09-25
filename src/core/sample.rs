//! Sample slots, written by the control thread and read by the audio thread.
//!
//! Decoding happens on the control thread; the frames then travel down an
//! `rtrb` ring and are copied into a pool that was allocated before the stream
//! started. Nothing here allocates on the audio side, so the red-zone contract
//! holds: [`SamplePool::drain`] only writes into memory that already exists.

use rtrb::Consumer;

/// Distinct samples the audio thread can hold at once.
pub const MAX_SAMPLE_SLOTS: usize = 8;
/// Frames one slot holds: about ten seconds at 48 kHz.
pub const MAX_SAMPLE_FRAMES: usize = 480_000;
/// Capacity of the sample ring, in messages.
///
/// A load sends one message per frame plus a header and a tail, and the control
/// thread pushes a whole file in one go, so the ring has to hold the largest
/// load whole. A ring smaller than this silently refused every sample longer
/// than about a second and a half.
pub const SAMPLE_RING_CAPACITY: usize = MAX_SAMPLE_FRAMES + 16;
/// A `slot` value meaning "no sample".
pub const NO_SLOT: u8 = u8::MAX;

/// One message down the sample ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SampleMsg {
    /// A load is starting: the next `frames` messages are this slot's audio.
    Begin { slot: u8, frames: u32, rate: f32 },
    /// One mono frame.
    Frame(f32),
    /// The load is complete; the slot becomes playable.
    End { slot: u8 },
    /// Give the slot's memory back.
    Free { slot: u8 },
}

/// A load in progress.
#[derive(Clone, Copy, Debug)]
struct Loading {
    slot: u8,
    total: u32,
    written: u32,
}

/// The audio thread's sample storage.
pub struct SamplePool {
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
    /// Allocate every slot up front, so the render path never grows the pool.
    pub fn new() -> Self {
        Self {
            data: vec![0.0; MAX_SAMPLE_SLOTS * MAX_SAMPLE_FRAMES],
            lengths: [0; MAX_SAMPLE_SLOTS],
            rates: [1.0; MAX_SAMPLE_SLOTS],
            loading: None,
        }
    }

    /// Take everything the control thread has pushed. Allocation-free.
    pub fn drain(&mut self, consumer: &mut Consumer<SampleMsg>) {
        while let Ok(message) = consumer.pop() {
            self.apply(message);
        }
    }

    /// Apply a single message, for callers that hold one at a time.
    pub fn push(&mut self, message: SampleMsg) {
        self.apply(message);
    }

    fn apply(&mut self, message: SampleMsg) {
        match message {
            SampleMsg::Begin { slot, frames, rate } => {
                let slot = slot as usize;
                self.loading = Some(Loading {
                    slot: slot as u8,
                    total: frames.min(MAX_SAMPLE_FRAMES as u32),
                    written: 0,
                });
                self.lengths[slot] = 0;
                self.rates[slot] = if rate > 0.0 { rate } else { 1.0 };
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
            SampleMsg::End { slot } => {
                let slot = slot as usize;
                if let Some(loading) = self.loading.take() {
                    self.lengths[slot] = loading.written;
                }
            }
            SampleMsg::Free { slot } => {
                let slot = slot as usize;
                if slot < MAX_SAMPLE_SLOTS {
                    self.lengths[slot] = 0;
                }
            }
        }
    }

    /// The slot's frames, empty until its load completes.
    pub fn frames(&self, slot: u8) -> &[f32] {
        let slot = slot as usize;
        if slot >= MAX_SAMPLE_SLOTS {
            return &[];
        }
        let length = self.lengths[slot] as usize;
        let start = slot * MAX_SAMPLE_FRAMES;
        self.data.get(start..start + length).unwrap_or(&[])
    }

    /// The rate the slot's frames are at, as a ratio to the engine's.
    pub fn rate(&self, slot: u8) -> f32 {
        let slot = slot as usize;
        if slot >= MAX_SAMPLE_SLOTS {
            return 0.0;
        }
        self.rates[slot]
    }

    /// Whether the slot holds a usable sample.
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
            producer.push(*message).expect("ring should have room");
        }
        pool.drain(&mut consumer);
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
