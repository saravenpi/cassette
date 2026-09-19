//! A single slot in a track's sequence.

/// One step of a track. A step either sounds a note or rests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Whether the step triggers a note.
    pub active: bool,
    /// MIDI note number.
    pub pitch: u8,
    /// MIDI velocity, `1..=127`.
    pub velocity: u8,
}

impl Step {
    /// A silent step at C4.
    pub const EMPTY: Step = Step {
        active: false,
        pitch: 60,
        velocity: 100,
    };

    /// An active step with the given pitch and velocity.
    pub const fn on(pitch: u8, velocity: u8) -> Self {
        Self {
            active: true,
            pitch,
            velocity,
        }
    }

    /// A resting step that remembers the given pitch, so toggling it on keeps it.
    pub const fn rest(pitch: u8) -> Self {
        Self {
            active: false,
            pitch,
            velocity: 100,
        }
    }

    /// Flip between sounding and resting.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips_active() {
        let mut step = Step::EMPTY;
        step.toggle();
        assert!(step.active);
        step.toggle();
        assert!(!step.active);
    }

    #[test]
    fn rest_keeps_pitch() {
        assert_eq!(Step::rest(48).pitch, 48);
        assert!(!Step::rest(48).active);
    }
}
