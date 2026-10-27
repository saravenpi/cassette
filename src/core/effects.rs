//! The built-in insert effects, as allocation-free stereo processors.
//!
//! [`TrackEffects`] holds one instance of every [`EffectKind`] for a track, with
//! the delay and reverb buffers sized at construction for the engine's sample
//! rate. [`TrackEffects::process`], [`TrackEffects::set_chain`] and
//! [`TrackEffects::set_tempo`] run in the red zone: they never allocate, lock,
//! do I/O or panic.

use crate::core::MAX_TRACKS;
use crate::models::{Effect, EffectChain, EffectKind, MAX_EFFECTS};
use rtrb::Producer;

const SILENCE: f32 = 1e-5;
const DENORMAL: f32 = 1e-20;
const SMOOTH_SECONDS: f32 = 0.01;
const GLIDE_SECONDS: f32 = 0.05;
const TAIL_HOLD_SECONDS: f32 = 0.1;
const DELAY_SECONDS: f32 = 4.0;
const DEFAULT_BPM: f32 = 120.0;
const REFERENCE_RATE: f32 = 44_100.0;
const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;
const FIXED_GAIN: f32 = 0.015;
const ALLPASS_FEEDBACK: f32 = 0.5;
const SCALE_WET: f32 = 3.0;
const SCALE_DAMP: f32 = 0.4;
const SCALE_ROOM: f32 = 0.28;
const OFFSET_ROOM: f32 = 0.7;
const MIN_Q: f32 = 0.5;
const MAX_Q: f32 = 12.0;
const EQ_MID_Q: f64 = 0.7;
const SHELF_Q: f64 = std::f64::consts::FRAC_1_SQRT_2;

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn flush(x: f32) -> f32 {
    if x.abs() < DENORMAL { 0.0 } else { x }
}

fn smoothing(seconds: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sample_rate).max(1.0)).exp()
}

fn ms_coefficient(ms: f32, sample_rate: f32) -> f32 {
    (-1.0 / (ms * 0.001 * sample_rate).max(1.0)).exp()
}

/// Parameter `index` of `effect`, clamped to its spec, or the default when it
/// is not a finite number.
fn param(effect: &Effect, index: usize) -> f32 {
    let Some(spec) = effect.kind.spec().get(index) else {
        return 0.0;
    };
    match effect.params.get(index) {
        Some(value) if value.is_finite() => value.clamp(spec.min, spec.max),
        _ => spec.default,
    }
}

/// A value that glides toward its target with a one-pole filter.
#[derive(Clone, Copy, Debug)]
struct Smoothed {
    value: f32,
    target: f32,
}

impl Smoothed {
    const fn at(value: f32) -> Self {
        Self {
            value,
            target: value,
        }
    }

    fn set(&mut self, target: f32) {
        self.target = target;
    }

    fn snap(&mut self) {
        self.value = self.target;
    }

    fn next(&mut self, k: f32) -> f32 {
        self.value += (self.target - self.value) * k;
        self.value
    }
}

/// Which RBJ cookbook response a biquad takes.
#[derive(Clone, Copy, Debug)]
enum Shape {
    LowPass,
    HighPass,
    BandPass,
    Peak,
    LowShelf,
    HighShelf,
}

/// Normalised `[b0, b1, b2, a1, a2]` for one RBJ cookbook biquad.
fn rbj(shape: Shape, freq: f32, q: f32, gain_db: f32, sample_rate: f32) -> [f32; 5] {
    let nyquist = f64::from(sample_rate) * 0.5;
    let freq = f64::from(freq).clamp(10.0, nyquist * 0.9);
    let w0 = std::f64::consts::TAU * freq / f64::from(sample_rate);
    let (sin, cos) = w0.sin_cos();
    let alpha = sin / (2.0 * f64::from(q).max(0.01));
    let a = 10f64.powf(f64::from(gain_db) / 40.0);
    let root = 2.0 * a.sqrt() * alpha;
    let [b0, b1, b2, a0, a1, a2] = match shape {
        Shape::LowPass => [
            (1.0 - cos) / 2.0,
            1.0 - cos,
            (1.0 - cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ],
        Shape::HighPass => [
            (1.0 + cos) / 2.0,
            -(1.0 + cos),
            (1.0 + cos) / 2.0,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        ],
        Shape::BandPass => [alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        Shape::Peak => [
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        ],
        Shape::LowShelf => [
            a * ((a + 1.0) - (a - 1.0) * cos + root),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - root),
            (a + 1.0) + (a - 1.0) * cos + root,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - root,
        ],
        Shape::HighShelf => [
            a * ((a + 1.0) + (a - 1.0) * cos + root),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - root),
            (a + 1.0) - (a - 1.0) * cos + root,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - root,
        ],
    };
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0].map(|c| c as f32)
}

/// One transposed direct form II section on one channel.
fn tick(state: &mut [f32; 2], c: &[f32; 5], x: f32) -> f32 {
    let y = c[0] * x + state[0];
    state[0] = flush(c[1] * x - c[3] * y + state[1]);
    state[1] = flush(c[2] * x - c[4] * y);
    y
}

/// A stereo biquad whose coefficients glide toward their target, so a moving
/// cutoff or gain does not click.
#[derive(Clone, Copy, Debug)]
struct Biquad {
    coeffs: [f32; 5],
    target: [f32; 5],
    left: [f32; 2],
    right: [f32; 2],
}

impl Biquad {
    const IDENTITY: Biquad = Biquad {
        coeffs: [1.0, 0.0, 0.0, 0.0, 0.0],
        target: [1.0, 0.0, 0.0, 0.0, 0.0],
        left: [0.0; 2],
        right: [0.0; 2],
    };

    fn reset(&mut self) {
        self.coeffs = self.target;
        self.left = [0.0; 2];
        self.right = [0.0; 2];
    }

    fn process(&mut self, l: f32, r: f32, k: f32) -> (f32, f32) {
        for (c, t) in self.coeffs.iter_mut().zip(self.target) {
            *c += (t - *c) * k;
        }
        (
            tick(&mut self.left, &self.coeffs, l),
            tick(&mut self.right, &self.coeffs, r),
        )
    }
}

/// EQ Three: a low shelf, a mid peak and a high shelf.
#[derive(Clone, Copy, Debug)]
/// An EQ with low shelf, peak mid and high shelf bands.
pub struct EqThree {
    bands: [Biquad; 3],
}

impl EqThree {
    fn configure(&mut self, effect: &Effect, sample_rate: f32) {
        let (low, mid, high) = (param(effect, 0), param(effect, 1), param(effect, 2));
        let (low_freq, high_freq) = (param(effect, 3), param(effect, 4));
        let mid_freq = (low_freq * high_freq).sqrt();
        let [lo, mi, hi] = &mut self.bands;
        lo.target = rbj(Shape::LowShelf, low_freq, SHELF_Q as f32, low, sample_rate);
        mi.target = rbj(Shape::Peak, mid_freq, EQ_MID_Q as f32, mid, sample_rate);
        hi.target = rbj(
            Shape::HighShelf,
            high_freq,
            SHELF_Q as f32,
            high,
            sample_rate,
        );
    }

    fn reset(&mut self) {
        self.bands.iter_mut().for_each(Biquad::reset);
    }

    fn process(&mut self, l: f32, r: f32, k: f32) -> (f32, f32) {
        self.bands
            .iter_mut()
            .fold((l, r), |(l, r), band| band.process(l, r, k))
    }
}

/// Filter: a resonant low-, high- or band-pass biquad.
#[derive(Clone, Copy, Debug)]
/// A multi-mode biquad filter.
pub struct Filter {
    biquad: Biquad,
}

impl Filter {
    fn configure(&mut self, effect: &Effect, sample_rate: f32) {
        let shape = match param(effect, 0).round() as u8 {
            1 => Shape::HighPass,
            2 => Shape::BandPass,
            _ => Shape::LowPass,
        };
        let q = MIN_Q * (MAX_Q / MIN_Q).powf(param(effect, 2));
        self.biquad.target = rbj(shape, param(effect, 1), q, 0.0, sample_rate);
    }

    fn reset(&mut self) {
        self.biquad.reset();
    }

    fn process(&mut self, l: f32, r: f32, k: f32) -> (f32, f32) {
        self.biquad.process(l, r, k)
    }
}

/// Saturator: pre-gain into a tanh soft clip, then output gain and dry/wet.
#[derive(Clone, Copy, Debug)]
/// A soft-clipping saturation stage.
pub struct Saturator {
    drive: Smoothed,
    output: Smoothed,
    mix: Smoothed,
}

impl Saturator {
    fn configure(&mut self, effect: &Effect) {
        self.drive.set(db_to_gain(param(effect, 0)));
        self.output.set(db_to_gain(param(effect, 1)));
        self.mix.set(param(effect, 2));
    }

    fn reset(&mut self) {
        self.drive.snap();
        self.output.snap();
        self.mix.snap();
    }

    fn process(&mut self, l: f32, r: f32, k: f32) -> (f32, f32) {
        let drive = self.drive.next(k);
        let output = self.output.next(k);
        let mix = self.mix.next(k);
        let shape = |x: f32| x + ((x * drive).tanh() * output - x) * mix;
        (shape(l), shape(r))
    }
}

/// Compressor: a feed-forward peak detector linked across both channels.
#[derive(Clone, Copy, Debug)]
/// A downward compressor with sidechain support.
pub struct Compressor {
    threshold: f32,
    slope: f32,
    attack: f32,
    release: f32,
    makeup: Smoothed,
    envelope: f32,
}

impl Compressor {
    fn configure(&mut self, effect: &Effect, sample_rate: f32) {
        self.threshold = param(effect, 0);
        self.slope = 1.0 - 1.0 / param(effect, 1).max(1.0);
        self.attack = ms_coefficient(param(effect, 2), sample_rate);
        self.release = ms_coefficient(param(effect, 3), sample_rate);
        self.makeup.set(param(effect, 4));
    }

    fn reset(&mut self) {
        self.makeup.snap();
        self.envelope = 0.0;
    }

    fn process(&mut self, l: f32, r: f32, k: f32, sidechain_key: Option<(f32, f32)>) -> (f32, f32) {
        let (key_l, key_r) = sidechain_key.unwrap_or((l, r));
        let level = key_l.abs().max(key_r.abs());
        let coefficient = if level > self.envelope {
            self.attack
        } else {
            self.release
        };
        self.envelope = flush(level + (self.envelope - level) * coefficient);
        let db = 20.0 * self.envelope.max(1e-9).log10();
        let reduction = (db - self.threshold).max(0.0) * self.slope;
        let gain = db_to_gain(self.makeup.next(k) - reduction);
        (l * gain, r * gain)
    }
}

/// The sample `delay` frames behind `write` in a ring, linearly interpolated.
fn read_behind(buffer: &[f32], write: usize, delay: f32) -> f32 {
    let len = buffer.len();
    let whole = (delay as usize).min(len.saturating_sub(2));
    let frac = delay - whole as f32;
    let near = (write + len - whole) % len.max(1);
    let far = (write + len - whole - 1) % len.max(1);
    let a = buffer.get(near).copied().unwrap_or(0.0);
    let b = buffer.get(far).copied().unwrap_or(0.0);
    a + (b - a) * frac
}

/// Delay: a tempo-synced ping-pong echo with a gliding time.
pub struct Delay {
    left: Box<[f32]>,
    right: Box<[f32]>,
    write: usize,
    beats: f32,
    time: Smoothed,
    feedback: Smoothed,
    mix: Smoothed,
    glide: f32,
}

impl Delay {
    fn new(sample_rate: f32) -> Self {
        let len = (sample_rate * DELAY_SECONDS).ceil() as usize + 4;
        Self {
            left: vec![0.0; len].into_boxed_slice(),
            right: vec![0.0; len].into_boxed_slice(),
            write: 0,
            beats: 0.0,
            time: Smoothed::at(1.0),
            feedback: Smoothed::at(0.0),
            mix: Smoothed::at(0.0),
            glide: smoothing(GLIDE_SECONDS, sample_rate),
        }
    }

    fn configure(&mut self, effect: &Effect, frames_per_beat: f32) {
        self.beats = param(effect, 0);
        self.feedback.set(param(effect, 1));
        self.mix.set(param(effect, 2));
        self.retime(frames_per_beat);
    }

    fn retime(&mut self, frames_per_beat: f32) {
        let longest = self.left.len().saturating_sub(2) as f32;
        self.time
            .set((self.beats * frames_per_beat).clamp(1.0, longest.max(1.0)));
    }

    fn longest_frames(&self) -> f32 {
        self.time.value.max(self.time.target)
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
        self.time.snap();
        self.feedback.snap();
        self.mix.snap();
    }

    fn process(&mut self, l: f32, r: f32, k: f32) -> (f32, f32) {
        let time = self.time.next(self.glide);
        let feedback = self.feedback.next(k);
        let mix = self.mix.next(k);
        let echo_l = read_behind(&self.left, self.write, time);
        let echo_r = read_behind(&self.right, self.write, time);
        if let Some(slot) = self.left.get_mut(self.write) {
            *slot = flush(0.5 * (l + r) + echo_r * feedback);
        }
        if let Some(slot) = self.right.get_mut(self.write) {
            *slot = flush(echo_l * feedback);
        }
        self.write += 1;
        if self.write >= self.left.len() {
            self.write = 0;
        }
        (l + (echo_l - l) * mix, r + (echo_r - r) * mix)
    }
}

/// A Freeverb lowpass-feedback comb filter.
struct Comb {
    buffer: Box<[f32]>,
    index: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)].into_boxed_slice(),
            index: 0,
            store: 0.0,
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.store = 0.0;
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let Some(slot) = self.buffer.get_mut(self.index) else {
            return 0.0;
        };
        let output = *slot;
        self.store = flush(output * (1.0 - damp) + self.store * damp);
        *slot = flush(input + self.store * feedback);
        self.index += 1;
        if self.index >= self.buffer.len() {
            self.index = 0;
        }
        output
    }
}

/// A Freeverb Schroeder allpass with a fixed feedback of one half.
struct Allpass {
    buffer: Box<[f32]>,
    index: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)].into_boxed_slice(),
            index: 0,
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
    }

    fn process(&mut self, input: f32) -> f32 {
        let Some(slot) = self.buffer.get_mut(self.index) else {
            return input;
        };
        let stored = *slot;
        *slot = flush(input + stored * ALLPASS_FEEDBACK);
        self.index += 1;
        if self.index >= self.buffer.len() {
            self.index = 0;
        }
        stored - input
    }
}

/// One channel of Freeverb: eight parallel combs into four series allpasses.
struct FreeverbChannel {
    combs: [Comb; 8],
    allpasses: [Allpass; 4],
}

impl FreeverbChannel {
    fn new(sample_rate: f32, spread: usize) -> Self {
        let scale = |len: usize| ((len + spread) as f32 * sample_rate / REFERENCE_RATE).round();
        Self {
            combs: COMB_TUNINGS.map(|len| Comb::new(scale(len) as usize)),
            allpasses: ALLPASS_TUNINGS.map(|len| Allpass::new(scale(len) as usize)),
        }
    }

    fn reset(&mut self) {
        self.combs.iter_mut().for_each(Comb::reset);
        self.allpasses.iter_mut().for_each(Allpass::reset);
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let wet: f32 = self
            .combs
            .iter_mut()
            .map(|comb| comb.process(input, feedback, damp))
            .sum();
        self.allpasses
            .iter_mut()
            .fold(wet, |signal, allpass| allpass.process(signal))
    }
}

/// Reverb: Freeverb with a stereo spread of 23 samples at 44.1 kHz.
///
/// Reference: https://ccrma.stanford.edu/~jos/pasp/Freeverb.html
/// Freeverb-style stereo reverberator.
pub struct Reverb {
    left: FreeverbChannel,
    right: FreeverbChannel,
    feedback: f32,
    damp: f32,
    mix: Smoothed,
    sample_rate: f32,
}

impl Reverb {
    fn new(sample_rate: f32) -> Self {
        Self {
            left: FreeverbChannel::new(sample_rate, 0),
            right: FreeverbChannel::new(sample_rate, STEREO_SPREAD),
            feedback: OFFSET_ROOM,
            damp: 0.0,
            mix: Smoothed::at(0.0),
            sample_rate,
        }
    }

    fn longest_frames(&self) -> f32 {
        self.sample_rate * 2.0
    }

    fn configure(&mut self, effect: &Effect) {
        self.feedback = OFFSET_ROOM + SCALE_ROOM * param(effect, 0);
        self.damp = SCALE_DAMP * param(effect, 1);
        self.mix.set(param(effect, 2));
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
        self.mix.snap();
    }

    fn process(&mut self, l: f32, r: f32, k: f32) -> (f32, f32) {
        let mix = self.mix.next(k);
        let input = (l + r) * FIXED_GAIN;
        let wet_l = self.left.process(input, self.feedback, self.damp);
        let wet_r = self.right.process(input, self.feedback, self.damp);
        let (dry, wet) = (1.0 - mix, mix * SCALE_WET);
        (l * dry + wet_l * wet, r * dry + wet_r * wet)
    }
}

/// Chorus placeholder.
pub struct Chorus;
impl Chorus {
    fn new(_sample_rate: f32) -> Self {
        Self
    }
    fn configure(&mut self, _effect: &Effect, _sample_rate: f32) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: f32, r: f32, _k: f32) -> (f32, f32) {
        (l, r)
    }
    fn longest_frames(&self) -> f32 {
        0.0
    }
}

/// Phaser placeholder.
pub struct Phaser;
impl Phaser {
    fn new(_sample_rate: f32) -> Self {
        Self
    }
    fn configure(&mut self, _effect: &Effect, _sample_rate: f32) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: f32, r: f32, _k: f32) -> (f32, f32) {
        (l, r)
    }
    fn longest_frames(&self) -> f32 {
        0.0
    }
}

/// AutoPan placeholder.
pub struct AutoPan;
impl AutoPan {
    fn new(_sample_rate: f32) -> Self {
        Self
    }
    fn configure(&mut self, _effect: &Effect, _frames_per_beat: f32) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: f32, r: f32, _k: f32) -> (f32, f32) {
        (l, r)
    }
}

/// BitCrusher placeholder.
pub struct BitCrusher;
impl BitCrusher {
    fn new() -> Self {
        Self
    }
    fn configure(&mut self, _effect: &Effect) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: f32, r: f32, _k: f32) -> (f32, f32) {
        (l, r)
    }
}

/// Utility placeholder.
pub struct Utility;
impl Utility {
    fn new() -> Self {
        Self
    }
    fn configure(&mut self, _effect: &Effect) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: f32, r: f32, _k: f32) -> (f32, f32) {
        (l, r)
    }
}

/// Limiter placeholder.
pub struct Limiter;
impl Limiter {
    fn new(_sample_rate: f32) -> Self {
        Self
    }
    fn configure(&mut self, _effect: &Effect, _sample_rate: f32) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: f32, r: f32, _k: f32) -> (f32, f32) {
        (l, r)
    }
}

/// An allocated instance of one insert effect.
#[allow(clippy::large_enum_variant)]
pub enum EffectInstance {
    Eq(EqThree),
    Filter(Filter),
    Saturator(Saturator),
    Compressor(Compressor),
    Delay(Delay),
    Reverb(Reverb),
    Chorus(Chorus),
    Phaser(Phaser),
    AutoPan(AutoPan),
    BitCrusher(BitCrusher),
    Utility(Utility),
    Limiter(Limiter),
}

impl EffectInstance {
    /// Allocate an effect instance for `kind` at `sample_rate`.
    pub fn new(kind: EffectKind, sample_rate: f32) -> Self {
        match kind {
            EffectKind::Eq => Self::Eq(EqThree {
                bands: [Biquad::IDENTITY; 3],
            }),
            EffectKind::Filter => Self::Filter(Filter {
                biquad: Biquad::IDENTITY,
            }),
            EffectKind::Saturator => Self::Saturator(Saturator {
                drive: Smoothed::at(1.0),
                output: Smoothed::at(1.0),
                mix: Smoothed::at(0.0),
            }),
            EffectKind::Compressor => Self::Compressor(Compressor {
                threshold: 0.0,
                slope: 0.0,
                attack: 0.0,
                release: 0.0,
                makeup: Smoothed::at(0.0),
                envelope: 0.0,
            }),
            EffectKind::Delay => Self::Delay(Delay::new(sample_rate)),
            EffectKind::Reverb => Self::Reverb(Reverb::new(sample_rate)),
            EffectKind::Chorus => Self::Chorus(Chorus::new(sample_rate)),
            EffectKind::Phaser => Self::Phaser(Phaser::new(sample_rate)),
            EffectKind::AutoPan => Self::AutoPan(AutoPan::new(sample_rate)),
            EffectKind::BitCrusher => Self::BitCrusher(BitCrusher::new()),
            EffectKind::Utility => Self::Utility(Utility::new()),
            EffectKind::Limiter => Self::Limiter(Limiter::new(sample_rate)),
        }
    }

    /// The kind of effect this instance represents.
    pub fn kind(&self) -> EffectKind {
        match self {
            Self::Eq(_) => EffectKind::Eq,
            Self::Filter(_) => EffectKind::Filter,
            Self::Saturator(_) => EffectKind::Saturator,
            Self::Compressor(_) => EffectKind::Compressor,
            Self::Delay(_) => EffectKind::Delay,
            Self::Reverb(_) => EffectKind::Reverb,
            Self::Chorus(_) => EffectKind::Chorus,
            Self::Phaser(_) => EffectKind::Phaser,
            Self::AutoPan(_) => EffectKind::AutoPan,
            Self::BitCrusher(_) => EffectKind::BitCrusher,
            Self::Utility(_) => EffectKind::Utility,
            Self::Limiter(_) => EffectKind::Limiter,
        }
    }

    /// Configure parameters from `effect`.
    pub fn configure(&mut self, effect: &Effect, sample_rate: f32, frames_per_beat: f32) {
        match self {
            Self::Eq(e) => e.configure(effect, sample_rate),
            Self::Filter(f) => f.configure(effect, sample_rate),
            Self::Saturator(s) => s.configure(effect),
            Self::Compressor(c) => c.configure(effect, sample_rate),
            Self::Delay(d) => d.configure(effect, frames_per_beat),
            Self::Reverb(r) => r.configure(effect),
            Self::Chorus(ch) => ch.configure(effect, sample_rate),
            Self::Phaser(p) => p.configure(effect, sample_rate),
            Self::AutoPan(ap) => ap.configure(effect, frames_per_beat),
            Self::BitCrusher(bc) => bc.configure(effect),
            Self::Utility(u) => u.configure(effect),
            Self::Limiter(lim) => lim.configure(effect, sample_rate),
        }
    }

    /// Reset internal filter state.
    pub fn reset(&mut self) {
        match self {
            Self::Eq(e) => e.reset(),
            Self::Filter(f) => f.reset(),
            Self::Saturator(s) => s.reset(),
            Self::Compressor(c) => c.reset(),
            Self::Delay(d) => d.reset(),
            Self::Reverb(r) => r.reset(),
            Self::Chorus(ch) => ch.reset(),
            Self::Phaser(p) => p.reset(),
            Self::AutoPan(ap) => ap.reset(),
            Self::BitCrusher(bc) => bc.reset(),
            Self::Utility(u) => u.reset(),
            Self::Limiter(lim) => lim.reset(),
        }
    }

    /// Process one stereo sample.
    pub fn process(
        &mut self,
        l: f32,
        r: f32,
        k: f32,
        sidechain_key: Option<(f32, f32)>,
    ) -> (f32, f32) {
        match self {
            Self::Eq(e) => e.process(l, r, k),
            Self::Filter(f) => f.process(l, r, k),
            Self::Saturator(s) => s.process(l, r, k),
            Self::Compressor(c) => c.process(l, r, k, sidechain_key),
            Self::Delay(d) => d.process(l, r, k),
            Self::Reverb(r_fx) => r_fx.process(l, r, k),
            Self::Chorus(ch) => ch.process(l, r, k),
            Self::Phaser(p) => p.process(l, r, k),
            Self::AutoPan(ap) => ap.process(l, r, k),
            Self::BitCrusher(bc) => bc.process(l, r, k),
            Self::Utility(u) => u.process(l, r, k),
            Self::Limiter(lim) => lim.process(l, r, k),
        }
    }

    /// Longest echo duration in frames for ringing check.
    pub fn longest_frames(&self) -> f32 {
        match self {
            Self::Delay(d) => d.longest_frames(),
            Self::Reverb(r) => r.longest_frames(),
            Self::Chorus(ch) => ch.longest_frames(),
            Self::Phaser(p) => p.longest_frames(),
            _ => 0.0,
        }
    }

    /// Whether this effect can have a tail.
    pub fn has_tail(&self) -> bool {
        self.kind().has_tail()
    }
}

/// A new effect instance handed to the audio thread.
pub struct EffectHandoff {
    pub track: u8,
    pub id: u32,
    pub instance: Box<EffectInstance>,
}

/// Capacity of the effect handoff and return ring buffers.
pub const EFFECT_RING_CAPACITY: usize = 256;

/// One populated slot in a track's insert chain.
pub struct EffectSlot {
    pub id: u32,
    pub effect: Effect,
    pub instance: Box<EffectInstance>,
}

/// One track's insert chain: up to [`MAX_EFFECTS`] running instances.
pub struct TrackEffects {
    sample_rate: f32,
    frames_per_beat: f32,
    smooth: f32,
    slots: [Option<EffectSlot>; MAX_EFFECTS],
    len: usize,
    incoming: [Option<EffectHandoff>; MAX_EFFECTS],
    quiet: u32,
    hold: u32,
}

impl TrackEffects {
    /// Construct empty track effects for `sample_rate`.
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        Self {
            sample_rate,
            frames_per_beat: sample_rate * 60.0 / DEFAULT_BPM,
            smooth: smoothing(SMOOTH_SECONDS, sample_rate),
            slots: std::array::from_fn(|_| None),
            len: 0,
            incoming: std::array::from_fn(|_| None),
            quiet: 0,
            hold: 0,
        }
    }

    /// Receive an allocated effect from the control thread into incoming staging.
    pub fn receive_handoff(
        &mut self,
        handoff: EffectHandoff,
        return_producer: Option<&mut Producer<Box<EffectInstance>>>,
    ) {
        for slot in &mut self.incoming {
            if slot.is_none() {
                *slot = Some(handoff);
                return;
            }
        }
        if let Some(prod) = return_producer
            && let Err(rtrb::PushError::Full(inst)) = prod.push(handoff.instance)
        {
            std::mem::forget(inst);
        }
    }

    /// Replace the chain with a new description.
    pub fn set_chain(&mut self, chain: &EffectChain) {
        self.set_chain_with_return(chain, None);
    }

    /// Replace the chain with a new description, pairing existing or incoming
    /// instances by id and returning removed instances to the return ring.
    pub fn set_chain_with_return(
        &mut self,
        chain: &EffectChain,
        mut return_producer: Option<&mut Producer<Box<EffectInstance>>>,
    ) {
        let count = (chain.len as usize).min(MAX_EFFECTS);
        let mut new_slots: [Option<EffectSlot>; MAX_EFFECTS] = std::array::from_fn(|_| None);

        for (i, effect) in chain.slots[..count].iter().enumerate() {
            let mut is_new = false;
            let mut found: Option<Box<EffectInstance>> = None;
            for old_slot in self.slots.iter_mut() {
                if let Some(slot) = old_slot
                    && slot.id == effect.id
                    && slot.instance.kind() == effect.kind
                {
                    found = Some(old_slot.take().unwrap().instance);
                    break;
                }
            }
            if found.is_none() {
                for inc in self.incoming.iter_mut() {
                    if let Some(h) = inc
                        && h.id == effect.id
                        && h.instance.kind() == effect.kind
                    {
                        found = Some(inc.take().unwrap().instance);
                        is_new = true;
                        break;
                    }
                }
            }
            if found.is_none() && return_producer.is_none() {
                let mut inst = Box::new(EffectInstance::new(effect.kind, self.sample_rate));
                inst.configure(effect, self.sample_rate, self.frames_per_beat);
                is_new = true;
                found = Some(inst);
            }
            if let Some(mut inst) = found {
                inst.configure(effect, self.sample_rate, self.frames_per_beat);
                if is_new {
                    inst.reset();
                }
                new_slots[i] = Some(EffectSlot {
                    id: effect.id,
                    effect: *effect,
                    instance: inst,
                });
            }
        }

        for old_slot in self.slots.iter_mut() {
            if let Some(slot) = old_slot.take()
                && let Some(ref mut prod) = return_producer
                && let Err(rtrb::PushError::Full(inst)) = prod.push(slot.instance)
            {
                std::mem::forget(inst);
            }
        }
        for inc in self.incoming.iter_mut() {
            if let Some(h) = inc.take()
                && let Some(ref mut prod) = return_producer
                && let Err(rtrb::PushError::Full(inst)) = prod.push(h.instance)
            {
                std::mem::forget(inst);
            }
        }

        self.slots = new_slots;
        self.len = count;
        self.update_hold();
    }

    /// Set the tempo the delay time follows, in beats per minute.
    pub fn set_tempo(&mut self, bpm: f32) {
        if !bpm.is_finite() || bpm <= 0.0 {
            return;
        }
        self.frames_per_beat = self.sample_rate * 60.0 / bpm;
        for slot in self.slots[..self.len].iter_mut().flatten() {
            slot.instance
                .configure(&slot.effect, self.sample_rate, self.frames_per_beat);
        }
        self.update_hold();
    }

    /// Run one stereo frame through the enabled effects, in chain order.
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.process_with_sidechain(l, r, &[(0.0, 0.0); MAX_TRACKS], 0, 1)
    }

    /// Run one stereo frame through the enabled effects, resolving sidechains
    /// against [`last_frame_outputs`].
    pub fn process_with_sidechain(
        &mut self,
        l: f32,
        r: f32,
        last_frame_outputs: &[(f32, f32); MAX_TRACKS],
        own_track: usize,
        track_count: usize,
    ) -> (f32, f32) {
        let k = self.smooth;
        let mut out = (l, r);
        for slot in self.slots[..self.len].iter_mut().flatten() {
            if !slot.effect.enabled {
                continue;
            }
            let key = if slot.effect.kind.takes_sidechain() {
                slot.effect.sidechain.and_then(|src| {
                    let s = src as usize;
                    if s != own_track && s < track_count {
                        Some(last_frame_outputs[s])
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            out = slot.instance.process(out.0, out.1, k, key);
        }
        let level = out.0.abs().max(out.1.abs()).max(l.abs()).max(r.abs());
        self.quiet = if level > SILENCE {
            0
        } else {
            self.quiet.saturating_add(1)
        };
        out
    }

    /// Whether a delay or reverb tail may still be audible.
    pub fn is_ringing(&self) -> bool {
        self.quiet < self.hold
    }

    fn update_hold(&mut self) {
        let mut max_echo = 0.0f32;
        let mut any_tail = false;
        for slot in self.slots[..self.len].iter().flatten() {
            if slot.effect.enabled && slot.instance.has_tail() {
                any_tail = true;
                max_echo = max_echo.max(slot.instance.longest_frames());
            }
        }
        self.hold = if any_tail {
            (self.sample_rate * TAIL_HOLD_SECONDS + max_echo) as u32
        } else {
            0
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44_100.0;

    fn effect(kind: EffectKind, params: &[f32]) -> Effect {
        let mut effect = Effect::new(kind);
        for (slot, value) in effect.params.iter_mut().zip(params) {
            *slot = *value;
        }
        effect
    }

    fn with(effects: &[Effect]) -> TrackEffects {
        let mut fx = TrackEffects::new(SR);
        fx.set_chain(&EffectChain::from_effects(effects));
        fx
    }

    fn sine(freq: f32, frame: usize) -> f32 {
        (std::f32::consts::TAU * freq * frame as f32 / SR).sin()
    }

    fn gain_at(fx: &mut TrackEffects, freq: f32, amplitude: f32) -> f32 {
        let frames = SR as usize;
        let mut sum = 0.0f32;
        for frame in 0..frames {
            let x = amplitude * sine(freq, frame);
            let (l, _) = fx.process(x, x);
            if frame >= frames / 2 {
                sum += l * l;
            }
        }
        let rms = (sum / (frames / 2) as f32).sqrt();
        rms / (amplitude * std::f32::consts::FRAC_1_SQRT_2)
    }

    #[test]
    fn a_low_pass_attenuates_a_high_sine() {
        let mut fx = with(&[effect(EffectKind::Filter, &[0.0, 500.0, 0.2])]);
        assert!(gain_at(&mut fx, 8_000.0, 0.5) < 0.05);
    }

    #[test]
    fn a_high_pass_attenuates_a_low_sine() {
        let mut fx = with(&[effect(EffectKind::Filter, &[1.0, 2_000.0, 0.2])]);
        assert!(gain_at(&mut fx, 100.0, 0.5) < 0.05);
    }

    #[test]
    fn a_band_pass_keeps_its_centre_and_cuts_the_sides() {
        let band = [effect(EffectKind::Filter, &[2.0, 1_000.0, 0.5])];
        assert!(gain_at(&mut with(&band), 1_000.0, 0.5) > 0.9);
        assert!(gain_at(&mut with(&band), 50.0, 0.5) < 0.1);
        assert!(gain_at(&mut with(&band), 15_000.0, 0.5) < 0.1);
    }

    #[test]
    fn a_cutoff_past_nyquist_stays_stable() {
        let mut fx = with(&[effect(EffectKind::Filter, &[0.0, 1.0e9, 1.0])]);
        let gain = gain_at(&mut fx, 440.0, 0.5);
        assert!(gain.is_finite() && gain < 2.0, "{gain}");
    }

    #[test]
    fn an_eq_low_boost_lifts_a_low_sine() {
        let mut fx = with(&[effect(EffectKind::Eq, &[12.0, 0.0, 0.0, 250.0, 2_500.0])]);
        let gain = gain_at(&mut fx, 60.0, 0.1);
        assert!(gain > 3.0, "+12 dB is about 4x, got {gain}");
    }

    #[test]
    fn a_flat_eq_leaves_a_sine_alone() {
        let mut fx = with(&[Effect::new(EffectKind::Eq)]);
        assert!((gain_at(&mut fx, 1_000.0, 0.5) - 1.0).abs() < 0.01);
    }

    #[test]
    fn the_saturator_bounds_its_output() {
        let mut fx = with(&[effect(EffectKind::Saturator, &[36.0, 0.0, 1.0])]);
        for frame in 0..4_410 {
            let x = 8.0 * sine(220.0, frame);
            let (l, r) = fx.process(x, x);
            assert!(l.abs() <= 1.0 && r.abs() <= 1.0, "{l} {r}");
        }
    }

    #[test]
    fn the_compressor_lowers_a_loud_peak() {
        let mut fx = with(&[effect(
            EffectKind::Compressor,
            &[-30.0, 10.0, 1.0, 100.0, 0.0],
        )]);
        let mut peak = 0.0f32;
        for frame in 0..SR as usize {
            let (l, _) = fx.process(sine(1_000.0, frame), sine(1_000.0, frame));
            if frame > SR as usize / 2 {
                peak = peak.max(l.abs());
            }
        }
        assert!(peak < 0.2, "{peak}");
    }

    #[test]
    fn the_delay_echoes_one_time_value_later() {
        let mut fx = TrackEffects::new(SR);
        fx.set_tempo(90.0);
        fx.set_chain(&EffectChain::from_effects(&[effect(
            EffectKind::Delay,
            &[0.5, 0.0, 1.0],
        )]));
        let expected = (0.5 * SR * 60.0 / 90.0) as usize;
        let mut first = None;
        for frame in 0..2 * expected {
            let x = if frame == 0 { 1.0 } else { 0.0 };
            let (l, _) = fx.process(x, x);
            if first.is_none() && l.abs() > 0.5 {
                first = Some(frame);
            }
        }
        assert_eq!(first, Some(expected));
    }

    #[test]
    fn the_delay_ping_pongs_to_the_right() {
        let mut fx = with(&[effect(EffectKind::Delay, &[0.5, 0.5, 1.0])]);
        let step = (0.5 * SR * 60.0 / DEFAULT_BPM) as usize;
        let mut right = vec![0.0f32; 3 * step];
        for (frame, out) in right.iter_mut().enumerate() {
            let x = if frame == 0 { 1.0 } else { 0.0 };
            *out = fx.process(x, x).1;
        }
        assert_eq!(right[step], 0.0, "the first echo is on the left");
        assert!(right[2 * step] > 0.4, "the second echo is on the right");
    }

    #[test]
    fn re_adding_the_delay_starts_from_silence() {
        let delay = effect(EffectKind::Delay, &[0.5, 0.9, 1.0]);
        let mut fx = with(&[delay]);
        for frame in 0..1_000 {
            let x = if frame == 0 { 1.0 } else { 0.0 };
            fx.process(x, x);
        }
        fx.set_chain(&EffectChain::EMPTY);
        fx.set_chain(&EffectChain::from_effects(&[delay]));
        let loudest = (0..SR as usize)
            .map(|_| fx.process(0.0, 0.0).0.abs())
            .fold(0.0, f32::max);
        assert_eq!(loudest, 0.0);
    }

    #[test]
    fn the_reverb_rings_after_an_impulse_then_falls_silent() {
        let mut fx = with(&[Effect::new(EffectKind::Reverb)]);
        let mut tail = 0.0f32;
        for frame in 0..SR as usize / 2 {
            let x = if frame == 0 { 1.0 } else { 0.0 };
            let (l, r) = fx.process(x, x);
            if frame > 2_000 {
                tail = tail.max(l.abs().max(r.abs()));
            }
        }
        assert!(tail > 1e-3, "{tail}");
        assert!(fx.is_ringing());
        for _ in 0..20 * SR as usize {
            fx.process(0.0, 0.0);
        }
        assert!(!fx.is_ringing());
    }

    #[test]
    fn disabled_effects_pass_audio_through_bit_exact() {
        let effects = EffectKind::ALL.map(|kind| Effect {
            enabled: false,
            ..Effect::new(kind)
        });
        let mut fx = with(&effects);
        for frame in 0..4_096 {
            let (l, r) = (0.7 * sine(333.0, frame), -0.3 * sine(71.0, frame));
            assert_eq!(fx.process(l, r), (l, r));
        }
        assert!(!fx.is_ringing());
    }

    #[test]
    fn an_empty_chain_is_bit_exact_and_never_rings() {
        let mut fx = TrackEffects::new(SR);
        for frame in 0..1_024 {
            let x = sine(440.0, frame);
            assert_eq!(fx.process(x, x), (x, x));
        }
        assert!(!fx.is_ringing());
    }

    #[test]
    fn every_effect_on_stays_finite_under_garbage_params() {
        let effects = EffectKind::ALL.map(|kind| Effect {
            kind,
            enabled: true,
            params: [f32::NAN, f32::INFINITY, -1.0e9, 1.0e9, f32::NEG_INFINITY],
            ..Effect::new(kind)
        });
        let mut fx = with(&effects);
        for frame in 0..SR as usize {
            let x = sine(440.0, frame);
            let (l, r) = fx.process(x, x);
            assert!(l.is_finite() && r.is_finite());
        }
    }

    #[test]
    fn two_delays_on_one_track_both_run() {
        let mut d1 = effect(EffectKind::Delay, &[0.25, 0.0, 1.0]);
        d1.id = 1;
        let mut d2 = effect(EffectKind::Delay, &[0.50, 0.0, 1.0]);
        d2.id = 2;
        let mut fx = with(&[d1, d2]);
        let mut echoes = Vec::new();
        for frame in 0..SR as usize {
            let x = if frame == 0 { 1.0 } else { 0.0 };
            let (l, _) = fx.process(x, x);
            if l.abs() > 0.05 {
                echoes.push(frame);
            }
        }
        assert!(echoes.len() >= 2);
    }

    #[test]
    fn a_reorder_keeps_a_reverb_tail() {
        let mut rev = Effect::new(EffectKind::Reverb);
        rev.id = 1;
        rev.params[0] = 0.9;
        rev.params[2] = 1.0;
        let mut sat = Effect::new(EffectKind::Saturator);
        sat.id = 2;
        let mut fx = with(&[rev, sat]);
        for frame in 0..1_000 {
            let x = if frame == 0 { 1.0 } else { 0.0 };
            fx.process(x, x);
        }
        assert!(fx.is_ringing());
        fx.set_chain(&EffectChain::from_effects(&[sat, rev]));
        assert!(fx.is_ringing());
        let mut tail = 0.0f32;
        for _ in 0..2_000 {
            let (l, r) = fx.process(0.0, 0.0);
            tail = tail.max(l.abs().max(r.abs()));
        }
        assert!(tail > 1e-4);
    }

    #[test]
    fn removal_returns_the_box_to_the_control_side() {
        let mut fx = TrackEffects::new(SR);
        let (mut return_prod, mut return_cons) = rtrb::RingBuffer::<Box<EffectInstance>>::new(16);
        let mut del = Effect::new(EffectKind::Delay);
        del.id = 10;
        let inst = Box::new(EffectInstance::new(EffectKind::Delay, SR));
        fx.receive_handoff(
            EffectHandoff {
                track: 0,
                id: 10,
                instance: inst,
            },
            Some(&mut return_prod),
        );
        fx.set_chain_with_return(&EffectChain::from_effects(&[del]), Some(&mut return_prod));
        assert_eq!(return_cons.slots(), 0);
        fx.set_chain_with_return(&EffectChain::EMPTY, Some(&mut return_prod));
        assert_eq!(return_cons.slots(), 1);
        let returned = return_cons.pop().expect("returned instance");
        assert_eq!(returned.kind(), EffectKind::Delay);
    }
}
