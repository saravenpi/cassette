use std::sync::atomic::{AtomicU64, Ordering};

pub struct Transport {
    pub position: AtomicU64,
    pub bpm: f32,
    pub sample_rate: u32,
    pub playing: bool,
    pub loop_start: Option<u64>,
    pub loop_end: Option<u64>,
}

impl Transport {
    pub fn new(sample_rate: u32, bpm: f32) -> Self {
        Self {
            position: AtomicU64::new(0),
            bpm,
            sample_rate,
            playing: false,
            loop_start: None,
            loop_end: None,
        }
    }

    pub fn advance_frame(&self) -> Option<u64> {
        if !self.playing {
            return None;
        }
        let pos = self.position.fetch_add(1, Ordering::Relaxed);
        if let (Some(start), Some(end)) = (self.loop_start, self.loop_end) {
            if pos >= end {
                self.position.store(start, Ordering::Relaxed);
                return Some(start);
            }
        }
        Some(pos)
    }

    pub fn position_frames(&self) -> u64 {
        self.position.load(Ordering::Relaxed)
    }

    pub fn position_ms(&self) -> f64 {
        (self.position_frames() as f64 / self.sample_rate as f64) * 1000.0
    }
}