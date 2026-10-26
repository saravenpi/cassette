//! Oscillator waveforms and the per-track instrument (timbre + envelope).

use serde::{Deserialize, Serialize};

/// The oscillator shape used by a track's voices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
    Noise,
}

impl Waveform {
    /// Every waveform, in menu order.
    pub const ALL: [Waveform; 5] = [
        Waveform::Sine,
        Waveform::Square,
        Waveform::Saw,
        Waveform::Triangle,
        Waveform::Noise,
    ];

    /// Short human-readable name.
    pub const fn name(self) -> &'static str {
        match self {
            Waveform::Sine => "Sine",
            Waveform::Square => "Square",
            Waveform::Saw => "Saw",
            Waveform::Triangle => "Triangle",
            Waveform::Noise => "Noise",
        }
    }
}

/// A subtractive-synthesis instrument: one oscillator plus an ADSR envelope.
///
/// Times are seconds; `sustain` is a level in `[0, 1]`; `octave` transposes the
/// incoming note before it reaches the oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub waveform: Waveform,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    /// Per-voice output gain, `[0, 1]`.
    pub gain: f32,
    /// Octave transposition applied to played notes.
    pub octave: i32,
}

impl Instrument {
    /// Build an instrument explicitly.
    pub const fn new(
        waveform: Waveform,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        gain: f32,
        octave: i32,
    ) -> Self {
        Self {
            waveform,
            attack,
            decay,
            sustain,
            release,
            gain,
            octave,
        }
    }

    /// Named instrument presets, in menu order.
    pub const PRESETS: [&'static str; 7] = ["Lead", "Bass", "Pad", "Pluck", "Kick", "Snare", "Hat"];

    /// Look up a preset by name. Unknown names fall back to `Lead`.
    pub fn preset(name: &str) -> Self {
        match name {
            "Bass" => Self::new(Waveform::Saw, 0.005, 0.18, 0.45, 0.10, 0.55, -1),
            "Pad" => Self::new(Waveform::Triangle, 0.25, 0.40, 0.80, 0.60, 0.30, 0),
            "Pluck" => Self::new(Waveform::Triangle, 0.002, 0.22, 0.0, 0.12, 0.50, 0),
            "Kick" => Self::new(Waveform::Sine, 0.001, 0.28, 0.0, 0.10, 0.90, -1),
            "Snare" => Self::new(Waveform::Noise, 0.001, 0.16, 0.0, 0.09, 0.55, 0),
            "Hat" => Self::new(Waveform::Noise, 0.001, 0.05, 0.0, 0.03, 0.35, 2),
            _ => Self::new(Waveform::Square, 0.010, 0.12, 0.60, 0.15, 0.45, 0),
        }
    }

    /// Index of this instrument in [`Instrument::PRESETS`], if it matches one.
    pub fn preset_index(&self) -> Option<usize> {
        Self::PRESETS
            .iter()
            .position(|name| Instrument::preset(name) == *self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_distinct_and_lookup_by_name() {
        assert_eq!(Instrument::preset("Bass"), Instrument::preset("Bass"));
        assert_ne!(Instrument::preset("Bass"), Instrument::preset("Lead"));
    }

    #[test]
    fn unknown_preset_falls_back_to_lead() {
        assert_eq!(Instrument::preset("nope"), Instrument::preset("Lead"));
    }
}
