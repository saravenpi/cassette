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
    Pulse,
    Noise,
}

impl Waveform {
    /// Every waveform, in menu order.
    pub const ALL: [Waveform; 6] = [
        Waveform::Sine,
        Waveform::Square,
        Waveform::Saw,
        Waveform::Triangle,
        Waveform::Pulse,
        Waveform::Noise,
    ];

    /// Short human-readable name.
    pub const fn name(self) -> &'static str {
        match self {
            Waveform::Sine => "Sine",
            Waveform::Square => "Square",
            Waveform::Saw => "Saw",
            Waveform::Triangle => "Triangle",
            Waveform::Pulse => "Pulse",
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
    pub const PRESETS: [&'static str; 25] = [
        "Lead",
        "Bass",
        "Pad",
        "Pluck",
        "Kick",
        "Snare",
        "Hat",
        "Supersaw",
        "Sub Bass",
        "Acid",
        "Keys",
        "Bell",
        "Organ",
        "Strings",
        "Brass",
        "808",
        "Tom",
        "Clap",
        "Rim",
        "Pulse Lead",
        "Reese",
        "EPiano",
        "Clav",
        "Marimba",
        "Choir",
    ];

    /// Presets mapped to their instrument group.
    pub const PRESET_GROUPS: [(&'static str, &'static str); 25] = [
        ("Lead", "Synth"),
        ("Bass", "Bass"),
        ("Pad", "Pad"),
        ("Pluck", "Synth"),
        ("Kick", "Drums"),
        ("Snare", "Drums"),
        ("Hat", "Drums"),
        ("Supersaw", "Synth"),
        ("Sub Bass", "Bass"),
        ("Acid", "Bass"),
        ("Keys", "Keys"),
        ("Bell", "Keys"),
        ("Organ", "Keys"),
        ("Strings", "Pad"),
        ("Brass", "Synth"),
        ("808", "Drums"),
        ("Tom", "Drums"),
        ("Clap", "Drums"),
        ("Rim", "Drums"),
        ("Pulse Lead", "Synth"),
        ("Reese", "Bass"),
        ("EPiano", "Keys"),
        ("Clav", "Keys"),
        ("Marimba", "Keys"),
        ("Choir", "Pad"),
    ];

    /// Instrument category for a preset name.
    pub const fn group(name: &str) -> &'static str {
        match name.as_bytes() {
            b"Lead" | b"Pluck" | b"Supersaw" | b"Brass" | b"Pulse Lead" => "Synth",
            b"Keys" | b"Bell" | b"Organ" | b"EPiano" | b"Clav" | b"Marimba" => "Keys",
            b"Bass" | b"Sub Bass" | b"Acid" | b"Reese" => "Bass",
            b"Pad" | b"Strings" | b"Choir" => "Pad",
            b"Kick" | b"Snare" | b"Hat" | b"808" | b"Tom" | b"Clap" | b"Rim" => "Drums",
            _ => "Synth",
        }
    }

    /// Look up a preset by name. Unknown names fall back to `Lead`.
    pub fn preset(name: &str) -> Self {
        match name {
            "Bass" => Self::new(Waveform::Saw, 0.005, 0.18, 0.45, 0.10, 0.55, -1),
            "Pad" => Self::new(Waveform::Triangle, 0.25, 0.40, 0.80, 0.60, 0.30, 0),
            "Pluck" => Self::new(Waveform::Triangle, 0.002, 0.22, 0.0, 0.12, 0.50, 0),
            "Kick" => Self {
                waveform: Waveform::Sine,
                attack: 0.001,
                decay: 0.28,
                sustain: 0.0,
                release: 0.08,
                gain: 0.95,
                octave: -2,
                unison: 1,
                detune: 0.0,
                sub: 0.25,
                cutoff: 4_000.0,
                resonance: 0.15,
                filter_env: 2.5,
                pitch_env: 36.0,
                pitch_decay: 0.026,
                fm_ratio: 4.0,
                fm_index: 0.45,
                enabled: true,
            },
            "Snare" => Self {
                cutoff: 5_000.0,
                resonance: 0.2,
                filter_env: 1.0,
                ..Self::new(Waveform::Noise, 0.001, 0.16, 0.0, 0.09, 0.55, 0)
            },
            "Hat" => Self {
                cutoff: 14_000.0,
                resonance: 0.1,
                ..Self::new(Waveform::Noise, 0.001, 0.05, 0.0, 0.03, 0.35, 2)
            },
            "Supersaw" => Self {
                waveform: Waveform::Saw,
                attack: 0.01,
                decay: 0.30,
                sustain: 0.70,
                release: 0.30,
                gain: 0.45,
                octave: 0,
                unison: 7,
                detune: 25.0,
                sub: 0.0,
                cutoff: 20_000.0,
                resonance: 0.0,
                filter_env: 0.0,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Sub Bass" => Self {
                waveform: Waveform::Sine,
                attack: 0.01,
                decay: 0.20,
                sustain: 0.80,
                release: 0.15,
                gain: 0.70,
                octave: -1,
                unison: 1,
                detune: 0.0,
                sub: 0.5,
                cutoff: 400.0,
                resonance: 0.1,
                filter_env: 0.5,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Acid" => Self {
                waveform: Waveform::Saw,
                attack: 0.005,
                decay: 0.20,
                sustain: 0.20,
                release: 0.10,
                gain: 0.55,
                octave: -1,
                unison: 1,
                detune: 0.0,
                sub: 0.0,
                cutoff: 600.0,
                resonance: 0.75,
                filter_env: 3.5,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Keys" => Self {
                waveform: Waveform::Sine,
                attack: 0.005,
                decay: 0.80,
                sustain: 0.30,
                release: 0.25,
                gain: 0.50,
                octave: 0,
                unison: 1,
                detune: 0.0,
                sub: 0.0,
                cutoff: 8_000.0,
                resonance: 0.1,
                filter_env: 1.0,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 1.0,
                fm_index: 1.8,
                enabled: true,
            },
            "Bell" => Self {
                waveform: Waveform::Sine,
                attack: 0.002,
                decay: 1.20,
                sustain: 0.0,
                release: 0.80,
                gain: 0.45,
                octave: 1,
                unison: 1,
                detune: 0.0,
                sub: 0.0,
                cutoff: 12_000.0,
                resonance: 0.0,
                filter_env: 0.0,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 3.5,
                fm_index: 2.5,
                enabled: true,
            },
            "Organ" => Self {
                waveform: Waveform::Sine,
                attack: 0.005,
                decay: 0.05,
                sustain: 1.0,
                release: 0.05,
                gain: 0.45,
                octave: 0,
                unison: 3,
                detune: 4.0,
                sub: 0.6,
                cutoff: 10_000.0,
                resonance: 0.0,
                filter_env: 0.0,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 2.0,
                fm_index: 0.4,
                enabled: true,
            },
            "Strings" => Self {
                waveform: Waveform::Saw,
                attack: 0.35,
                decay: 0.50,
                sustain: 0.75,
                release: 0.60,
                gain: 0.40,
                octave: 0,
                unison: 5,
                detune: 18.0,
                sub: 0.0,
                cutoff: 5_000.0,
                resonance: 0.2,
                filter_env: 0.8,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Brass" => Self {
                waveform: Waveform::Saw,
                attack: 0.08,
                decay: 0.35,
                sustain: 0.55,
                release: 0.20,
                gain: 0.45,
                octave: 0,
                unison: 3,
                detune: 12.0,
                sub: 0.2,
                cutoff: 1_200.0,
                resonance: 0.3,
                filter_env: 2.5,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "808" => Self {
                waveform: Waveform::Sine,
                attack: 0.001,
                decay: 0.60,
                sustain: 0.0,
                release: 0.30,
                gain: 0.85,
                octave: -2,
                unison: 1,
                detune: 0.0,
                sub: 0.0,
                cutoff: 800.0,
                resonance: 0.0,
                filter_env: 0.0,
                pitch_env: 24.0,
                pitch_decay: 0.04,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Tom" => Self {
                waveform: Waveform::Sine,
                attack: 0.001,
                decay: 0.25,
                sustain: 0.0,
                release: 0.12,
                gain: 0.75,
                octave: -1,
                unison: 1,
                detune: 0.0,
                sub: 0.0,
                cutoff: 2_000.0,
                resonance: 0.1,
                filter_env: 0.0,
                pitch_env: 16.0,
                pitch_decay: 0.06,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Clap" => Self {
                waveform: Waveform::Noise,
                attack: 0.001,
                decay: 0.14,
                sustain: 0.0,
                release: 0.08,
                gain: 0.60,
                octave: 0,
                unison: 1,
                detune: 0.0,
                sub: 0.0,
                cutoff: 2_200.0,
                resonance: 0.4,
                filter_env: 1.5,
                pitch_env: 0.0,
                pitch_decay: 0.05,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Rim" => Self {
                waveform: Waveform::Triangle,
                attack: 0.001,
                decay: 0.04,
                sustain: 0.0,
                release: 0.03,
                gain: 0.65,
                octave: 1,
                unison: 1,
                detune: 0.0,
                sub: 0.0,
                cutoff: 4_000.0,
                resonance: 0.5,
                filter_env: 2.0,
                pitch_env: 12.0,
                pitch_decay: 0.015,
                fm_ratio: 1.0,
                fm_index: 0.0,
                enabled: true,
            },
            "Pulse Lead" => Self {
                unison: 2,
                detune: 8.0,
                cutoff: 2_400.0,
                resonance: 0.25,
                filter_env: 2.0,
                ..Self::new(Waveform::Pulse, 0.005, 0.20, 0.60, 0.15, 0.45, 0)
            },
            "Reese" => Self {
                unison: 5,
                detune: 32.0,
                sub: 0.6,
                cutoff: 450.0,
                resonance: 0.25,
                filter_env: 1.0,
                ..Self::new(Waveform::Saw, 0.01, 0.30, 0.80, 0.20, 0.40, -1)
            },
            "EPiano" => Self {
                cutoff: 6_000.0,
                resonance: 0.1,
                filter_env: 0.8,
                fm_ratio: 2.0,
                fm_index: 1.2,
                ..Self::new(Waveform::Sine, 0.004, 1.40, 0.15, 0.40, 0.50, 0)
            },
            "Clav" => Self {
                cutoff: 1_800.0,
                resonance: 0.35,
                filter_env: 3.0,
                ..Self::new(Waveform::Pulse, 0.001, 0.18, 0.0, 0.08, 0.50, 0)
            },
            "Marimba" => Self {
                cutoff: 5_000.0,
                filter_env: 1.0,
                fm_ratio: 4.0,
                fm_index: 0.8,
                ..Self::new(Waveform::Sine, 0.001, 0.22, 0.0, 0.15, 0.55, 0)
            },
            "Choir" => Self {
                unison: 5,
                detune: 16.0,
                cutoff: 1_100.0,
                resonance: 0.45,
                filter_env: 0.6,
                ..Self::new(Waveform::Pulse, 0.40, 0.50, 0.80, 0.70, 0.35, 0)
            },
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
        for (i, name) in Instrument::PRESETS.iter().enumerate() {
            let inst = Instrument::preset(name);
            assert_eq!(inst.preset_index(), Some(i));
            for other_name in Instrument::PRESETS.iter().skip(i + 1) {
                assert_ne!(inst, Instrument::preset(other_name));
            }
        }
    }

    #[test]
    fn unknown_preset_falls_back_to_lead() {
        assert_eq!(Instrument::preset("nope"), Instrument::preset("Lead"));
    }

    #[test]
    fn preset_groups_match_group_function() {
        for &(name, group_name) in &Instrument::PRESET_GROUPS {
            assert_eq!(Instrument::group(name), group_name);
        }
    }
}
