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

/// A subtractive-synthesis instrument: an oscillator with optional unison,
/// sub oscillator and FM, a resonant low-pass with its own envelope sweep, a
/// pitch sweep for drums, and an ADSR envelope.
///
/// Times are seconds; `sustain` is a level in `[0, 1]`; `octave` transposes the
/// incoming note before it reaches the oscillator. Fields after `octave` were
/// added in v0.15.0 and default to a plain, open voice, so older projects load
/// and sound as they did.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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
    /// Oscillators per voice, `1..=7`, spread by `detune`.
    pub unison: u8,
    /// Spread of the unison oscillators, in cents, `0..=100`.
    pub detune: f32,
    /// Level of a sine one octave below the oscillator, `[0, 1]`.
    pub sub: f32,
    /// Low-pass cutoff in Hz, `20..=20000`; `20000` leaves the voice open.
    pub cutoff: f32,
    /// Filter resonance, `[0, 1]`.
    pub resonance: f32,
    /// How far the envelope opens the filter above `cutoff`, in octaves, `0..=8`.
    pub filter_env: f32,
    /// Pitch sweep at note start, in semitones, `0..=48`, falling to the note.
    pub pitch_env: f32,
    /// Time the pitch sweep takes to fall, in seconds.
    pub pitch_decay: f32,
    /// Frequency ratio of the FM modulator to the carrier.
    pub fm_ratio: f32,
    /// FM modulation index, `0..=10`; `0` turns FM off.
    pub fm_index: f32,
    /// Whether the instrument sounds. A bypassed instrument is silent.
    pub enabled: bool,
}

impl Default for Instrument {
    fn default() -> Self {
        Self::preset("Lead")
    }
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
            unison: 1,
            detune: 0.0,
            sub: 0.0,
            cutoff: 20_000.0,
            resonance: 0.0,
            filter_env: 0.0,
            pitch_env: 0.0,
            pitch_decay: 0.05,
            fm_ratio: 1.0,
            fm_index: 0.0,
            enabled: true,
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
