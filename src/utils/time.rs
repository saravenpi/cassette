pub struct Tempo {
    bpm: f32,
    signature: (u8, u8),
}

impl Tempo {
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm,
            signature: (4, 4),
        }
    }

    pub fn beat_duration(&self) -> f32 {
        60.0 / self.bpm
    }
}
