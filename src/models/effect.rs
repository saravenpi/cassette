//! Insert effects on a track: the kinds the engine builds in, their parameter
//! tables, and the chain a track carries.

use serde::{Deserialize, Serialize};

/// Most parameters one effect exposes.
pub const MAX_PARAMS: usize = 5;
/// Most effects one track's chain holds: one of each kind.
pub const MAX_EFFECTS: usize = EffectKind::ALL.len();

/// A built-in insert effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectKind {
    Eq,
    Filter,
    Saturator,
    Compressor,
    Delay,
    Reverb,
}

/// How a parameter's number reads in the interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParamUnit {
    Decibels,
    Hertz,
    Milliseconds,
    Beats,
    Ratio,
    Percent,
    /// An index into [`ParamSpec::choices`].
    Choice,
}

/// One parameter of an effect: its range, its default and how it reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamSpec {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: ParamUnit,
    /// Whether a dial moves along a logarithmic scale.
    pub log: bool,
    pub choices: &'static [&'static str],
}

const fn param(
    name: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: ParamUnit,
    log: bool,
) -> ParamSpec {
    ParamSpec {
        name,
        min,
        max,
        default,
        unit,
        log,
        choices: &[],
    }
}

const EQ: [ParamSpec; 5] = [
    param("Low", -24.0, 12.0, 0.0, ParamUnit::Decibels, false),
    param("Mid", -24.0, 12.0, 0.0, ParamUnit::Decibels, false),
    param("High", -24.0, 12.0, 0.0, ParamUnit::Decibels, false),
    param("Low freq", 50.0, 1_000.0, 250.0, ParamUnit::Hertz, true),
    param(
        "High freq",
        1_000.0,
        12_000.0,
        2_500.0,
        ParamUnit::Hertz,
        true,
    ),
];

const FILTER: [ParamSpec; 3] = [
    ParamSpec {
        name: "Type",
        min: 0.0,
        max: 2.0,
        default: 0.0,
        unit: ParamUnit::Choice,
        log: false,
        choices: &["Low-pass", "High-pass", "Band-pass"],
    },
    param("Cutoff", 20.0, 20_000.0, 2_000.0, ParamUnit::Hertz, true),
    param("Resonance", 0.0, 1.0, 0.2, ParamUnit::Percent, false),
];

const SATURATOR: [ParamSpec; 3] = [
    param("Drive", 0.0, 36.0, 6.0, ParamUnit::Decibels, false),
    param("Output", -24.0, 6.0, 0.0, ParamUnit::Decibels, false),
    param("Mix", 0.0, 1.0, 1.0, ParamUnit::Percent, false),
];

const COMPRESSOR: [ParamSpec; 5] = [
    param("Threshold", -60.0, 0.0, -18.0, ParamUnit::Decibels, false),
    param("Ratio", 1.0, 20.0, 4.0, ParamUnit::Ratio, true),
    param("Attack", 0.1, 100.0, 10.0, ParamUnit::Milliseconds, true),
    param(
        "Release",
        10.0,
        1_000.0,
        100.0,
        ParamUnit::Milliseconds,
        true,
    ),
    param("Makeup", 0.0, 24.0, 0.0, ParamUnit::Decibels, false),
];

const DELAY: [ParamSpec; 3] = [
    param("Time", 0.125, 2.0, 0.75, ParamUnit::Beats, false),
    param("Feedback", 0.0, 0.95, 0.4, ParamUnit::Percent, false),
    param("Mix", 0.0, 1.0, 0.3, ParamUnit::Percent, false),
];

const REVERB: [ParamSpec; 3] = [
    param("Size", 0.0, 1.0, 0.5, ParamUnit::Percent, false),
    param("Damping", 0.0, 1.0, 0.5, ParamUnit::Percent, false),
    param("Mix", 0.0, 1.0, 0.25, ParamUnit::Percent, false),
];

impl EffectKind {
    /// Every kind, in the order the effect browser lists them.
    pub const ALL: [EffectKind; 6] = [
        EffectKind::Eq,
        EffectKind::Filter,
        EffectKind::Saturator,
        EffectKind::Compressor,
        EffectKind::Delay,
        EffectKind::Reverb,
    ];

    /// Name shown on the device's title bar.
    pub const fn name(self) -> &'static str {
        match self {
            EffectKind::Eq => "EQ Three",
            EffectKind::Filter => "Filter",
            EffectKind::Saturator => "Saturator",
            EffectKind::Compressor => "Compressor",
            EffectKind::Delay => "Delay",
            EffectKind::Reverb => "Reverb",
        }
    }

    /// The parameters this kind exposes, in dial order.
    pub const fn spec(self) -> &'static [ParamSpec] {
        match self {
            EffectKind::Eq => &EQ,
            EffectKind::Filter => &FILTER,
            EffectKind::Saturator => &SATURATOR,
            EffectKind::Compressor => &COMPRESSOR,
            EffectKind::Delay => &DELAY,
            EffectKind::Reverb => &REVERB,
        }
    }

    /// Whether the effect keeps sounding after its input falls silent.
    pub const fn has_tail(self) -> bool {
        matches!(self, EffectKind::Delay | EffectKind::Reverb)
    }
}

/// One effect in a track's chain. Parameters past the kind's spec are unused.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub kind: EffectKind,
    pub enabled: bool,
    pub params: [f32; MAX_PARAMS],
}

impl Effect {
    /// A fresh, enabled effect at its default settings.
    pub fn new(kind: EffectKind) -> Self {
        let mut params = [0.0; MAX_PARAMS];
        for (slot, spec) in params.iter_mut().zip(kind.spec()) {
            *slot = spec.default;
        }
        Self {
            kind,
            enabled: true,
            params,
        }
    }
}

/// A track's chain as the audio thread receives it: fixed-size and `Copy`, so
/// it crosses the command ring without an allocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectChain {
    pub len: u8,
    pub slots: [Effect; MAX_EFFECTS],
}

impl EffectChain {
    /// A chain with no effects: audio passes straight through.
    pub const EMPTY: EffectChain = EffectChain {
        len: 0,
        slots: [Effect {
            kind: EffectKind::Eq,
            enabled: false,
            params: [0.0; MAX_PARAMS],
        }; MAX_EFFECTS],
    };

    /// Copy a track's effects into a chain, dropping any past [`MAX_EFFECTS`].
    pub fn from_effects(effects: &[Effect]) -> Self {
        let mut chain = Self::EMPTY;
        for (slot, effect) in chain.slots.iter_mut().zip(effects) {
            *slot = *effect;
            chain.len += 1;
        }
        chain
    }

    /// The effects in signal order.
    pub fn effects(&self) -> &[Effect] {
        &self.slots[..self.len as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_sits_inside_its_range() {
        for kind in EffectKind::ALL {
            assert!(kind.spec().len() <= MAX_PARAMS, "{kind:?}");
            for spec in kind.spec() {
                assert!(
                    spec.min <= spec.default && spec.default <= spec.max,
                    "{kind:?} {}",
                    spec.name
                );
                if spec.unit == ParamUnit::Choice {
                    assert_eq!(spec.choices.len(), spec.max as usize + 1);
                }
            }
        }
    }

    #[test]
    fn a_chain_keeps_the_order_it_was_given() {
        let effects = [Effect::new(EffectKind::Reverb), Effect::new(EffectKind::Eq)];
        let chain = EffectChain::from_effects(&effects);
        assert_eq!(chain.effects(), &effects);
    }
}
