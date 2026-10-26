//! A pre-allocated, allocation-free polyphonic synthesizer.
//!
//! Voices are stored in fixed-size arrays so the audio callback only ever
//! mutates existing memory. Envelope increments are derived from the
//! [`Instrument`] and the sample rate whenever an instrument changes.

use rtrb::Consumer;

use crate::core::effects::TrackEffects;
use crate::core::sample::{MAX_SAMPLE_SLOTS, NO_SLOT, SampleMsg, SamplePool};
use crate::core::{MAX_TRACKS, MAX_VOICES};
use crate::models::{EffectChain, Instrument, Waveform};
use crate::utils::frequency::note_to_freq;

const TWO_PI: f32 = std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Per-sample envelope increments, precomputed once per instrument.
#[derive(Clone, Copy, Debug)]
struct EnvRates {
    attack: f32,
    decay: f32,
    release: f32,
}

impl EnvRates {
    fn from(instrument: &Instrument, sample_rate: f32) -> Self {
        let attack = if instrument.attack > 1e-6 {
            1.0 / (instrument.attack * sample_rate)
        } else {
            1.0
        };
        let decay = if instrument.decay > 1e-6 {
            (1.0 - instrument.sustain).max(0.0) / (instrument.decay * sample_rate)
        } else {
            1.0
        };
        let release = if instrument.release > 1e-6 {
            1.0 / (instrument.release * sample_rate)
        } else {
            1.0
        };
        Self {
            attack,
            decay,
            release,
        }
    }
}

/// A single oscillator + envelope. `gate_left` counts frames until release;
/// `HELD` means the note is held until an explicit note-off.
#[derive(Clone, Copy, Debug)]
struct Voice {
    stage: Stage,
    note: u8,
    velocity: f32,
    phase: f32,
    phase_inc: f32,
    env: f32,
    gate_left: u32,
}

const HELD: u32 = u32::MAX;

impl Voice {
    const IDLE: Voice = Voice {
        stage: Stage::Idle,
        note: 0,
        velocity: 0.0,
        phase: 0.0,
        phase_inc: 0.0,
        env: 0.0,
        gate_left: 0,
    };

    fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    fn start(&mut self, pitch: u8, velocity: u8, instrument: &Instrument, sr: f32, gate: u32) {
        let freq = note_to_freq(pitch as f32 + instrument.octave as f32 * 12.0);
        self.stage = Stage::Attack;
        self.note = pitch;
        self.velocity = velocity as f32 / 127.0;
        self.phase = 0.0;
        self.phase_inc = (freq / sr).clamp(0.0, 0.49);
        self.env = 0.0;
        self.gate_left = gate;
    }

    fn retune(&mut self, instrument: &Instrument, sr: f32) {
        if self.is_idle() {
            return;
        }
        let freq = note_to_freq(self.note as f32 + instrument.octave as f32 * 12.0);
        self.phase_inc = (freq / sr).clamp(0.0, 0.49);
    }

    /// Advance one sample. Returns the sample and whether the voice just ended.
    fn next_sample(
        &mut self,
        instrument: &Instrument,
        rates: &EnvRates,
        rng: &mut u32,
    ) -> (f32, bool) {
        if self.is_idle() {
            return (0.0, false);
        }

        // Gate countdown drives the release stage.
        if self.gate_left != HELD {
            if self.gate_left == 0 {
                if self.stage != Stage::Release {
                    self.stage = Stage::Release;
                }
            } else {
                self.gate_left -= 1;
            }
        }

        match self.stage {
            Stage::Attack => {
                self.env += rates.attack;
                if self.env >= 1.0 {
                    self.env = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.env -= rates.decay;
                if self.env <= instrument.sustain {
                    self.env = instrument.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => {
                self.env = instrument.sustain;
            }
            Stage::Release => {
                self.env -= rates.release;
            }
            Stage::Idle => {}
        }

        self.phase += self.phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        let osc = waveform(instrument.waveform, self.phase, rng);
        let sample = osc * self.env * self.velocity;

        if self.stage == Stage::Release && self.env <= 0.0 {
            self.env = 0.0;
            self.stage = Stage::Idle;
            return (sample, true);
        }
        (sample, false)
    }
}

/// Where a sample voice enters its slot and how fast it runs.
///
/// `offset_seconds` is where the block starts reading its material and
/// `elapsed_frames` is how far, in engine frames, the playhead already is past
/// the block's start. `rate` scales the material's own speed, so a block
/// stretched to twice its length plays at half rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleCue {
    pub offset_seconds: f32,
    pub elapsed_frames: f64,
    pub rate: f32,
}

impl SampleCue {
    /// The top of the material, at its own speed.
    pub const START: SampleCue = SampleCue {
        offset_seconds: 0.0,
        elapsed_frames: 0.0,
        rate: 1.0,
    };
}

/// One track's sample voice: where its slot is playing, and how fast it moves.
///
/// The position is a phase in the slot's own frames, so a rate ratio of one
/// advances one source frame per engine frame. It is `f64` so a long sample
/// never loses the fractional part that the interpolation reads.
#[derive(Clone, Copy, Debug)]
struct SamplePlayer {
    slot: u8,
    position: f64,
    step: f64,
    gain: f32,
    pitch: u8,
    /// Which block started this voice, so a block's end cannot release another
    /// block's sample.
    tag: u8,
    /// Remaining amplitude of the release fade, 1.0 while held.
    level: f32,
    /// Per-frame change in `level`; negative once the voice is releasing.
    level_step: f32,
}

impl SamplePlayer {
    /// A player with nothing loaded.
    const IDLE: SamplePlayer = SamplePlayer {
        slot: NO_SLOT,
        position: 0.0,
        step: 1.0,
        gain: 0.0,
        pitch: 0,
        tag: 0,
        level: 1.0,
        level_step: 0.0,
    };

    /// Whether the player currently has a slot to read.
    fn playing(&self) -> bool {
        self.slot != NO_SLOT
    }
}

/// The interpolated frame the player sits on, or `None` once it has run out.
fn sample_at(player: &SamplePlayer, pool: &SamplePool) -> Option<f32> {
    let frames = pool.frames(player.slot);
    let at = player.position.floor();
    if at < 0.0 || at >= frames.len() as f64 {
        return None;
    }
    let at = at as usize;
    let next = (at + 1).min(frames.len() - 1);
    let frac = (player.position - at as f64) as f32;
    Some(frames[at] + (frames[next] - frames[at]) * frac)
}

/// One track's voices, insert chain and mixer state.
struct TrackVoices {
    voices: [Voice; MAX_VOICES],
    sample: SamplePlayer,
    effects: TrackEffects,
    instrument: Instrument,
    rates: EnvRates,
    volume: f32,
    pan: f32,
    muted: bool,
    solo: bool,
}

impl TrackVoices {
    fn new(instrument: Instrument, sample_rate: f32) -> Self {
        Self {
            voices: [Voice::IDLE; MAX_VOICES],
            sample: SamplePlayer::IDLE,
            effects: TrackEffects::new(sample_rate),
            instrument,
            rates: EnvRates::from(&instrument, sample_rate),
            volume: 0.8,
            pan: 0.0,
            muted: false,
            solo: false,
        }
    }

    fn set_instrument(&mut self, instrument: Instrument, sample_rate: f32) {
        self.instrument = instrument;
        self.rates = EnvRates::from(&instrument, sample_rate);
        for voice in &mut self.voices {
            voice.retune(&instrument, sample_rate);
        }
    }
}

/// Next pseudo-random sample in `[-1, 1]` from a xorshift32 state.
fn next_noise(rng: &mut u32) -> f32 {
    let mut x = *rng;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *rng = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn waveform(shape: Waveform, phase: f32, rng: &mut u32) -> f32 {
    match shape {
        Waveform::Sine => (phase * TWO_PI).sin(),
        Waveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Saw => 2.0 * phase - 1.0,
        Waveform::Triangle => {
            if phase < 0.5 {
                4.0 * phase - 1.0
            } else {
                3.0 - 4.0 * phase
            }
        }
        Waveform::Noise => next_noise(rng),
    }
}

/// Amplitude a sample voice plays at: the instrument's own gain.
///
/// The block's length does not enter here. What stops a sample longer than its
/// block is the timeline releasing the voice when the playhead passes the
/// block's end, which is a cut with a short fade rather than an amplitude the
/// engine guesses in advance.
pub fn sample_gain(instrument: &Instrument) -> f32 {
    instrument.gain.clamp(0.0, 1.0)
}

/// Frames a released sample voice takes to fade out, so a cut does not click.
pub const RELEASE_FADE_FRAMES: u32 = 128;

/// A fixed-array polyphonic synthesizer for up to [`MAX_TRACKS`] tracks.
///
/// Each track carries both its synth voices and one sample voice; the two are
/// summed into the track's mono mix.
pub struct SynthEngine {
    sample_rate: f32,
    tracks: [TrackVoices; MAX_TRACKS],
    track_count: usize,
    active_voices: usize,
    any_solo: bool,
    master: f32,
    rng: u32,
    pool: SamplePool,
    sample_consumer: Option<Consumer<SampleMsg>>,
    sample_ratios: [f32; MAX_SAMPLE_SLOTS],
}

impl SynthEngine {
    /// Build an engine with silent tracks and no active voices.
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        let default = Instrument::preset("Lead");
        Self {
            sample_rate: sr,
            tracks: std::array::from_fn(|_| TrackVoices::new(default, sr)),
            track_count: 0,
            active_voices: 0,
            any_solo: false,
            master: 0.85,
            rng: 0x1234_5678,
            pool: SamplePool::new(),
            sample_consumer: None,
            sample_ratios: [1.0; MAX_SAMPLE_SLOTS],
        }
    }

    /// Sample rate the engine was built for.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }

    /// Number of tracks currently audible.
    pub fn track_count(&self) -> usize {
        self.track_count
    }

    /// Set how many tracks are active (`<= MAX_TRACKS`).
    pub fn set_track_count(&mut self, count: usize) {
        let count = count.min(MAX_TRACKS);
        for track in &mut self.tracks[count..self.track_count.max(count)] {
            track.voices = [Voice::IDLE; MAX_VOICES];
            track.sample = SamplePlayer::IDLE;
            track.effects.set_chain(&EffectChain::EMPTY);
        }
        self.track_count = count;
        self.recompute_active_voices();
    }

    /// Replace a track's instrument and mixer settings.
    pub fn configure_track(
        &mut self,
        track: usize,
        instrument: Instrument,
        volume: f32,
        pan: f32,
        muted: bool,
        solo: bool,
    ) {
        if track >= MAX_TRACKS {
            return;
        }
        let sr = self.sample_rate;
        let t = &mut self.tracks[track];
        t.set_instrument(instrument, sr);
        t.volume = volume.clamp(0.0, 1.0);
        t.pan = pan.clamp(-1.0, 1.0);
        t.muted = muted;
        t.solo = solo;
        self.update_solo();
    }

    /// Replace a track's instrument while preserving its mixer settings.
    pub fn set_track_instrument(&mut self, track: usize, instrument: Instrument) {
        if track >= MAX_TRACKS {
            return;
        }
        let sr = self.sample_rate;
        self.tracks[track].set_instrument(instrument, sr);
    }

    /// Toggle a single mixer field without touching the instrument.
    pub fn set_track_mix(&mut self, track: usize, volume: f32, pan: f32, muted: bool, solo: bool) {
        if track >= MAX_TRACKS {
            return;
        }
        let t = &mut self.tracks[track];
        t.volume = volume.clamp(0.0, 1.0);
        t.pan = pan.clamp(-1.0, 1.0);
        t.muted = muted;
        t.solo = solo;
        self.update_solo();
    }

    /// Hand the engine the control thread's end of the sample ring.
    ///
    /// [`SynthEngine::drain_samples`] then applies the messages to the slots.
    /// Without a consumer the ring is not read, and samples arrive only through
    /// [`SynthEngine::push_sample`].
    pub fn set_sample_ring(&mut self, consumer: Consumer<SampleMsg>) {
        self.sample_consumer = Some(consumer);
    }

    /// Apply everything waiting on the sample ring to the slots.
    ///
    /// Allocation-free: a load swaps in a handle the sender keeps a clone of.
    pub fn drain_samples(&mut self) {
        let SynthEngine {
            pool,
            sample_consumer,
            ..
        } = self;
        if let Some(consumer) = sample_consumer.as_mut() {
            pool.drain(consumer);
        }
    }

    /// Apply one sample message straight to the pool, for tests and for a
    /// control side that feeds the engine a message at a time.
    pub fn push_sample(&mut self, message: SampleMsg) {
        self.pool.push(message);
    }

    /// Tell the engine what rate a slot's frames were decoded at.
    ///
    /// The engine keeps the ratio of that rate to its own. A WAV at another
    /// rate therefore plays pitch-shifted: 44.1 kHz material in a 48 kHz session
    /// runs slightly fast and high. That is Ableton's default Repitch warp mode,
    /// and it is what this engine does on purpose, not a bug.
    pub fn set_sample_rate(&mut self, slot: u8, rate: f32) {
        let slot = slot as usize;
        if slot >= MAX_SAMPLE_SLOTS {
            return;
        }
        let engine_rate = self.sample_rate.max(1.0);
        self.sample_ratios[slot] = if rate > 0.0 { rate / engine_rate } else { 1.0 };
    }

    /// Start a sample voice on `track`, entered into `slot` where `cue` says.
    ///
    /// Returns `false`, changing nothing, when the track is not one of the
    /// engine's, when the slot is [`NO_SLOT`], when the slot holds no finished
    /// load, or when `gain` is not positive. A track holds one sample voice, so
    /// a new trigger replaces the one already playing; that voice is summed with
    /// the track's synth voices rather than replacing them.
    pub fn play_sample(
        &mut self,
        track: usize,
        pitch: u8,
        slot: u8,
        cue: SampleCue,
        gain: f32,
        tag: u8,
    ) -> bool {
        if track >= self.track_count || slot == NO_SLOT || gain <= 0.0 || !self.pool.is_ready(slot)
        {
            return false;
        }
        let ratio = f64::from(self.sample_ratios[slot as usize]);
        let rate = if cue.rate.is_finite() && cue.rate > 0.0 {
            f64::from(cue.rate)
        } else {
            1.0
        };
        let slot_rate = ratio * f64::from(self.sample_rate);
        let offset = f64::from(cue.offset_seconds.max(0.0)) * slot_rate;
        let position = offset + cue.elapsed_frames.max(0.0) * ratio * rate;
        let player = &mut self.tracks[track].sample;
        let was_idle = !player.playing();
        *player = SamplePlayer {
            slot,
            position,
            step: ratio * rate,
            gain,
            pitch,
            tag,
            level: 1.0,
            level_step: 0.0,
        };
        if was_idle {
            self.active_voices += 1;
        }
        true
    }

    /// Begin the release fade on `track`'s sample voice, if `tag` started it.
    ///
    /// The timeline calls this when the playhead passes a block's end, so a
    /// sample longer than its block is cut with the block rather than running
    /// on. A mismatch is ignored, which is what keeps a block ending from
    /// silencing the sample a later block started.
    pub fn release_sample(&mut self, track: usize, tag: u8) -> bool {
        if track >= self.track_count {
            return false;
        }
        let player = &mut self.tracks[track].sample;
        if !player.playing() || player.tag != tag || player.level_step < 0.0 {
            return false;
        }
        player.level_step = -1.0 / RELEASE_FADE_FRAMES as f32;
        true
    }

    /// Re-tag `track`'s sample voice, after the lane re-sorted under it.
    pub fn retag_sample(&mut self, track: usize, tag: u8) {
        if track < self.track_count {
            self.tracks[track].sample.tag = tag;
        }
    }

    /// Begin the release fade on `track`'s sample voice, whichever block
    /// started it.
    pub fn release_playing_sample(&mut self, track: usize) {
        if track >= self.track_count {
            return;
        }
        let tag = self.tracks[track].sample.tag;
        self.release_sample(track, tag);
    }

    pub fn is_sample_playing(&self, track: usize) -> bool {
        if track >= self.track_count {
            return false;
        }
        self.tracks[track].sample.playing()
    }

    pub fn sample_slots_ready(&self) -> usize {
        (0..MAX_SAMPLE_SLOTS as u8)
            .filter(|slot| self.pool.is_ready(*slot))
            .count()
    }

    fn update_solo(&mut self) {
        self.any_solo = self.tracks[..self.track_count].iter().any(|t| t.solo);
    }

    fn recompute_active_voices(&mut self) {
        self.active_voices = self.tracks[..self.track_count]
            .iter()
            .map(|t| {
                t.voices.iter().filter(|v| !v.is_idle()).count() + usize::from(t.sample.playing())
            })
            .sum();
    }

    /// Replace one track's insert chain. Allocation-free.
    pub fn set_effects(&mut self, track: usize, chain: &EffectChain) {
        if let Some(t) = self.tracks.get_mut(track) {
            t.effects.set_chain(chain);
        }
    }

    /// Set the tempo every track's tempo-synced delay follows. Allocation-free.
    pub fn set_tempo(&mut self, bpm: f32) {
        for track in &mut self.tracks {
            track.effects.set_tempo(bpm);
        }
    }

    /// Trigger a note. `gate` is the number of frames before release, or
    /// `HELD` to sustain until [`SynthEngine::note_off`].
    pub fn note_on(&mut self, track: usize, pitch: u8, velocity: u8, gate: u32) {
        if track >= self.track_count || velocity == 0 {
            return;
        }
        let sr = self.sample_rate;
        let t = &mut self.tracks[track];
        let instrument = t.instrument;

        // Preference order: idle voice, then same-note retrigger, then steal
        // the quietest voice.
        let slot = t
            .voices
            .iter()
            .position(|v| v.is_idle())
            .or_else(|| {
                t.voices
                    .iter()
                    .position(|v| !v.is_idle() && v.note == pitch)
            })
            .unwrap_or_else(|| {
                let mut best = 0;
                let mut best_env = f32::MAX;
                for (i, v) in t.voices.iter().enumerate() {
                    if v.env < best_env {
                        best_env = v.env;
                        best = i;
                    }
                }
                best
            });

        let was_idle = t.voices[slot].is_idle();
        t.voices[slot].start(pitch, velocity, &instrument, sr, gate);
        if was_idle {
            self.active_voices += 1;
        }
    }

    /// Release any voices playing `pitch` on `track`.
    ///
    /// A sample voice the timeline triggered carries
    /// [`crate::core::render::EVENT_PITCH`], which no keyboard sends, so it is
    /// never released this way; a sample player started at a real pitch is.
    pub fn note_off(&mut self, track: usize, pitch: u8) {
        if track >= self.track_count {
            return;
        }
        for voice in &mut self.tracks[track].voices {
            if !voice.is_idle() && voice.note == pitch {
                voice.gate_left = 0;
            }
        }
        let player = &mut self.tracks[track].sample;
        if player.playing() && player.pitch == pitch {
            *player = SamplePlayer::IDLE;
            self.active_voices = self.active_voices.saturating_sub(1);
        }
    }

    /// Release every synth voice and fade out every sample voice.
    pub fn all_notes_off(&mut self) {
        for track in &mut self.tracks[..self.track_count] {
            for voice in &mut track.voices {
                if !voice.is_idle() {
                    voice.gate_left = 0;
                }
            }
        }
        self.release_all_samples();
        self.recompute_active_voices();
    }

    /// Begin the release fade on every playing sample voice.
    fn release_all_samples(&mut self) {
        for track in &mut self.tracks[..self.track_count] {
            let player = &mut track.sample;
            if player.playing() && player.level_step >= 0.0 {
                player.level_step = -1.0 / RELEASE_FADE_FRAMES as f32;
            }
        }
    }

    /// Whether any voice is currently producing sound.
    pub fn is_active(&self) -> bool {
        self.active_voices > 0
    }

    /// Whether any track's delay or reverb tail may still be audible.
    pub fn is_ringing(&self) -> bool {
        self.tracks[..self.track_count]
            .iter()
            .any(|t| t.effects.is_ringing())
    }

    /// Render one stereo frame, accumulating per-track peaks into `peaks`.
    ///
    /// Returns `(left, right)` already clamped to `[-1, 1]`. Each track's sample
    /// voice, when it has one, is summed into the track's mono mix after the
    /// synth voices. The mono mix runs through the track's insert chain as a
    /// stereo pair, the meter reads the chain's output before the fader, and
    /// then volume, instrument gain and balance pan apply.
    ///
    /// Muted and non-solo tracks still run their chain and discard the result,
    /// so a tail keeps its state across a mute and a meter keeps moving.
    /// Rendering continues past the last voice while any chain still rings.
    pub fn process_frame(&mut self, peaks: &mut [f32; MAX_TRACKS]) -> (f32, f32) {
        if self.active_voices == 0 && !self.is_ringing() {
            return (0.0, 0.0);
        }

        let SynthEngine {
            tracks,
            track_count,
            any_solo,
            master,
            rng,
            active_voices,
            pool,
            ..
        } = self;

        let mut sum_l = 0.0f32;
        let mut sum_r = 0.0f32;
        let mut finished = 0usize;

        for ti in 0..*track_count {
            let track = &mut tracks[ti];
            let instrument = track.instrument;
            let rates = track.rates;
            let mut mono = 0.0f32;
            for voice in &mut track.voices {
                let (sample, done) = voice.next_sample(&instrument, &rates, rng);
                if done {
                    finished += 1;
                }
                mono += sample;
            }

            if track.sample.playing() {
                match sample_at(&track.sample, pool) {
                    Some(frame) => {
                        track.sample.level =
                            (track.sample.level + track.sample.level_step).min(1.0);
                        if track.sample.level <= 0.0 {
                            track.sample = SamplePlayer::IDLE;
                            finished += 1;
                        } else {
                            mono += frame * track.sample.gain * track.sample.level;
                            track.sample.position += track.sample.step;
                            let end = pool.frames(track.sample.slot).len() as f64;
                            if track.sample.position >= end {
                                track.sample = SamplePlayer::IDLE;
                                finished += 1;
                            }
                        }
                    }
                    None => {
                        track.sample = SamplePlayer::IDLE;
                        finished += 1;
                    }
                }
            }

            let (l, r) = track.effects.process(mono, mono);
            let magnitude = l.abs().max(r.abs());
            if magnitude > peaks[ti] {
                peaks[ti] = magnitude;
            }

            let audible = !track.muted && (!*any_solo || track.solo);
            if audible {
                let gain = track.volume * instrument.gain;
                let pan = track.pan;
                sum_l += l * gain * (0.5 * (1.0 - pan));
                sum_r += r * gain * (0.5 * (1.0 + pan));
            }
        }

        *active_voices = active_voices.saturating_sub(finished);
        let master = *master;
        (
            (sum_l * master).clamp(-1.0, 1.0),
            (sum_r * master).clamp(-1.0, 1.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Effect, EffectKind, Instrument, Waveform};
    use rtrb::RingBuffer;
    use std::sync::Arc;

    /// Pitch the timeline triggers sample voices at, as `render.rs` does.
    const EVENT: u8 = 255;

    fn engine() -> SynthEngine {
        let mut e = SynthEngine::new(44_100);
        e.set_track_count(1);
        e.configure_track(0, Instrument::preset("Lead"), 1.0, 0.0, false, false);
        e
    }

    fn tracks(count: usize) -> SynthEngine {
        let mut e = SynthEngine::new(44_100);
        e.set_track_count(count);
        for track in 0..count {
            e.configure_track(track, Instrument::preset("Lead"), 1.0, 0.0, false, false);
        }
        e
    }

    fn load(e: &mut SynthEngine, slot: u8, frames: &[f32], rate: f32) {
        e.push_sample(SampleMsg::Load {
            slot,
            frames: Arc::from(frames),
            rate,
        });
    }

    fn constant(e: &mut SynthEngine, slot: u8, count: usize, value: f32) {
        load(e, slot, &vec![value; count], 44_100.0);
    }

    fn run(e: &mut SynthEngine, frames: usize) -> [f32; MAX_TRACKS] {
        let mut peaks = [0.0; MAX_TRACKS];
        for _ in 0..frames {
            e.process_frame(&mut peaks);
        }
        peaks
    }

    #[test]
    fn silence_until_a_note_is_triggered() {
        let mut e = engine();
        let mut peaks = [0.0; MAX_TRACKS];
        let (l, r) = e.process_frame(&mut peaks);
        assert_eq!((l, r), (0.0, 0.0));
    }

    #[test]
    fn a_note_produces_audio_and_shows_on_the_meter() {
        let mut e = engine();
        e.note_on(0, 69, 100, HELD);
        let mut peak = 0.0f32;
        let mut peaks = [0.0; MAX_TRACKS];
        for _ in 0..2_000 {
            let (l, _) = e.process_frame(&mut peaks);
            peak = peak.max(l.abs());
        }
        assert!(peak > 0.01, "expected audible output, got {peak}");
        assert!(peaks[0] > 0.01);
    }

    #[test]
    fn gated_note_stops_after_the_gate() {
        let mut e = engine();
        e.note_on(0, 69, 100, 10);
        let mut peaks = [0.0; MAX_TRACKS];
        for _ in 0..44_100 {
            e.process_frame(&mut peaks);
        }
        assert!(!e.is_active(), "voice should have been released and freed");
    }

    #[test]
    fn note_off_releases_a_held_note() {
        let mut e = engine();
        e.note_on(0, 60, 100, HELD);
        let mut peaks = [0.0; MAX_TRACKS];
        e.process_frame(&mut peaks);
        assert!(e.is_active());
        e.note_off(0, 60);
        for _ in 0..44_100 {
            e.process_frame(&mut peaks);
        }
        assert!(!e.is_active());
    }

    #[test]
    fn muted_tracks_are_silent_but_still_metered() {
        let mut e = engine();
        e.configure_track(0, Instrument::preset("Lead"), 1.0, 0.0, true, false);
        e.note_on(0, 69, 120, HELD);
        let mut peaks = [0.0; MAX_TRACKS];
        let mut peak = 0.0f32;
        for _ in 0..500 {
            let (l, _) = e.process_frame(&mut peaks);
            peak = peak.max(l.abs());
        }
        assert_eq!(peak, 0.0);
        assert!(peaks[0] > 0.0, "meter reflects pre-mute level");
    }

    #[test]
    fn note_on_for_an_unknown_track_is_ignored() {
        let mut e = engine();
        e.note_on(9, 69, 100, HELD);
        assert!(!e.is_active());
    }

    #[test]
    fn pushed_slots_are_reported_ready() {
        let mut e = engine();
        assert_eq!(e.sample_slots_ready(), 0);
        constant(&mut e, 0, 8, 0.5);
        constant(&mut e, 1, 8, 0.5);
        assert_eq!(e.sample_slots_ready(), 2);
    }

    #[test]
    fn the_ring_feeds_the_pool() {
        let mut e = engine();
        let (mut producer, consumer) = RingBuffer::<SampleMsg>::new(16);
        e.set_sample_ring(consumer);
        producer
            .push(SampleMsg::Load {
                slot: 0,
                frames: Arc::from(&[0.5, 0.5][..]),
                rate: 44_100.0,
            })
            .expect("room for the load");
        e.drain_samples();
        assert_eq!(e.sample_slots_ready(), 1);
    }

    #[test]
    fn a_loaded_sample_sounds_on_its_track() {
        let mut e = engine();
        constant(&mut e, 0, 512, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let peaks = run(&mut e, 400);
        assert!(
            (peaks[0] - 0.5).abs() < 1e-6,
            "the sample reached the meter"
        );
        assert!(e.is_active(), "a sample voice keeps the engine awake");
    }

    #[test]
    fn a_sample_plays_only_on_its_own_track() {
        let mut e = tracks(2);
        constant(&mut e, 0, 512, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let mut peaks = [0.0; MAX_TRACKS];
        let mut other = 0.0f32;
        for _ in 0..400 {
            e.process_frame(&mut peaks);
            other = other.max(peaks[1]);
        }
        assert!(peaks[0] > 0.4, "the triggered track sounds");
        assert_eq!(other, 0.0, "the other track stays quiet");
    }

    #[test]
    fn an_empty_slot_is_refused_and_stays_silent() {
        let mut e = engine();
        assert!(
            !e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0),
            "nothing is loaded"
        );
        assert!(
            !e.play_sample(0, EVENT, NO_SLOT, SampleCue::START, 1.0, 0),
            "no slot at all"
        );
        assert!(
            !e.play_sample(9, EVENT, 0, SampleCue::START, 1.0, 0),
            "no such track"
        );
        constant(&mut e, 0, 512, 0.5);
        assert!(
            !e.play_sample(0, EVENT, 0, SampleCue::START, 0.0, 0),
            "silent gain"
        );
        assert!(!e.is_active());
        assert_eq!(run(&mut e, 64), [0.0; MAX_TRACKS]);
    }

    #[test]
    fn a_sample_enters_at_its_offset() {
        let mut e = engine();
        let mut frames = vec![0.25f32; 200];
        frames.extend_from_slice(&[0.75f32; 200]);
        load(&mut e, 0, &frames, 44_100.0);
        assert!(e.play_sample(
            0,
            EVENT,
            0,
            SampleCue {
                elapsed_frames: 200.0,
                ..SampleCue::START
            },
            1.0,
            0
        ));
        let peaks = run(&mut e, 1);
        assert!(
            (peaks[0] - 0.75).abs() < 1e-6,
            "the voice started on frame 200, got {}",
            peaks[0]
        );
    }

    #[test]
    fn the_rate_ratio_changes_how_fast_a_sample_runs() {
        let mut frames = vec![0.0f32; 100];
        frames.extend_from_slice(&[1.0f32; 100]);

        let mut fast = engine();
        fast.set_sample_rate(0, 44_100.0);
        load(&mut fast, 0, &frames, 44_100.0);
        assert!(fast.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let fast_peaks = run(&mut fast, 150);

        let mut slow = engine();
        slow.set_sample_rate(0, 22_050.0);
        load(&mut slow, 0, &frames, 44_100.0);
        assert!(slow.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let slow_peaks = run(&mut slow, 150);

        assert!(fast_peaks[0] > 0.5, "at engine rate it is half way through");
        assert_eq!(slow_peaks[0], 0.0, "at half rate it read half as far");
    }

    #[test]
    fn a_cue_offset_enters_the_material_in_seconds() {
        let mut frames = vec![0.0f32; 100];
        frames.extend_from_slice(&[1.0f32; 100]);
        let mut e = engine();
        load(&mut e, 0, &frames, 44_100.0);
        let cue = SampleCue {
            offset_seconds: 150.0 / 44_100.0,
            ..SampleCue::START
        };
        assert!(e.play_sample(0, EVENT, 0, cue, 1.0, 0));
        assert!(run(&mut e, 1)[0] > 0.5, "the voice started past frame 100");
    }

    #[test]
    fn a_cue_rate_scales_how_fast_the_material_runs() {
        let mut frames = vec![0.0f32; 100];
        frames.extend_from_slice(&[1.0f32; 100]);

        let mut double = engine();
        load(&mut double, 0, &frames, 44_100.0);
        let cue = SampleCue {
            rate: 2.0,
            ..SampleCue::START
        };
        assert!(double.play_sample(0, EVENT, 0, cue, 1.0, 0));
        let double_peaks = run(&mut double, 60);

        let mut normal = engine();
        load(&mut normal, 0, &frames, 44_100.0);
        assert!(normal.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let normal_peaks = run(&mut normal, 60);

        assert!(double_peaks[0] > 0.5, "at double rate it read 120 frames");
        assert_eq!(normal_peaks[0], 0.0, "at its own rate it read 60");
    }

    #[test]
    fn a_sample_and_a_synth_voice_on_one_track_sum() {
        let mut sample_only = engine();
        constant(&mut sample_only, 0, 512, 0.25);
        assert!(sample_only.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));

        let mut voice_only = engine();
        voice_only.note_on(0, 69, 100, HELD);

        let mut both = engine();
        constant(&mut both, 0, 512, 0.25);
        assert!(both.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        both.note_on(0, 69, 100, HELD);

        let sample_peak = run(&mut sample_only, 400)[0];
        let voice_peak = run(&mut voice_only, 400)[0];
        let sum_peak = run(&mut both, 400)[0];

        assert!(voice_peak > 0.01, "the synth voice sounds on its own");
        assert!(sample_peak > 0.24, "the sample sounds on its own");
        assert!(
            sum_peak > sample_peak,
            "the two add up: {sum_peak} against {sample_peak}"
        );
    }

    #[test]
    fn a_finished_sample_goes_idle_and_the_track_stops() {
        let mut e = engine();
        constant(&mut e, 0, 100, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        assert!(e.is_active());
        run(&mut e, 100);
        assert!(!e.is_active(), "the voice count was given back");
        assert_eq!(run(&mut e, 100), [0.0; MAX_TRACKS]);
    }

    #[test]
    fn all_notes_off_stops_a_sample_voice() {
        let mut e = engine();
        constant(&mut e, 0, 512, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        e.all_notes_off();
        assert!(e.is_active(), "the voice fades rather than cutting");
        run(&mut e, RELEASE_FADE_FRAMES as usize + 2);
        assert!(!e.is_active());
        let mut peaks = [0.0; MAX_TRACKS];
        assert_eq!(e.process_frame(&mut peaks), (0.0, 0.0));
    }

    #[test]
    fn a_new_trigger_replaces_the_playing_sample() {
        let mut e = engine();
        constant(&mut e, 0, 512, 0.25);
        constant(&mut e, 1, 512, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        assert!(e.play_sample(0, EVENT, 1, SampleCue::START, 1.0, 0));
        let peaks = run(&mut e, 1);
        assert!((peaks[0] - 0.5).abs() < 1e-6, "the newer slot is playing");
    }

    #[test]
    fn a_lower_track_count_releases_its_samples() {
        let mut e = tracks(2);
        constant(&mut e, 0, 512, 0.5);
        assert!(e.play_sample(1, EVENT, 0, SampleCue::START, 1.0, 0));
        e.set_track_count(1);
        assert!(!e.is_active());
        assert_eq!(run(&mut e, 32), [0.0; MAX_TRACKS]);
    }

    #[test]
    fn an_unloaded_slot_plays_nothing_once_it_is_freed() {
        let mut e = engine();
        constant(&mut e, 0, 512, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        e.push_sample(SampleMsg::Free { slot: 0 });
        assert_eq!(run(&mut e, 8), [0.0; MAX_TRACKS]);
        assert!(!e.is_active());
    }

    #[test]
    fn sample_gain_is_the_instrument_gain_and_ignores_block_length() {
        let instrument = Instrument::new(Waveform::Sine, 0.0, 0.1, 0.5, 2.0, 0.8, 0);
        assert!((sample_gain(&instrument) - 0.8).abs() < 1e-6);
        let loud = Instrument::new(Waveform::Sine, 0.0, 0.1, 0.5, 0.0, 1.0, 0);
        assert_eq!(sample_gain(&loud), 1.0);
    }

    #[test]
    fn releasing_a_sample_fades_it_out_and_frees_the_voice() {
        let mut e = engine();
        constant(&mut e, 0, 2_000, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 7));
        assert!(e.release_sample(0, 7));
        let mut peak_before = 0.0f32;
        let mut peaks = [0.0; MAX_TRACKS];
        let (l, _) = e.process_frame(&mut peaks);
        peak_before = peak_before.max(l.abs());
        assert!(peak_before > 0.0, "the fade starts loud, not silent");
        for _ in 0..RELEASE_FADE_FRAMES + 2 {
            e.process_frame(&mut peaks);
        }
        assert!(!e.is_active(), "a released sample voice is freed");
        assert!(peak_before > 0.0);
    }

    #[test]
    fn a_release_for_another_blocks_tag_is_ignored() {
        let mut e = engine();
        constant(&mut e, 0, 2_000, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 3));
        assert!(!e.release_sample(0, 4), "a different block's end");
        assert!(!e.release_sample(9, 3), "no such track");
        assert!(e.is_active());
    }

    #[test]
    fn releasing_a_sample_twice_is_a_no_op() {
        let mut e = engine();
        constant(&mut e, 0, 2_000, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 1));
        assert!(e.release_sample(0, 1));
        assert!(!e.release_sample(0, 1), "already releasing");
    }

    fn chain(effects: &[Effect]) -> EffectChain {
        EffectChain::from_effects(effects)
    }

    #[test]
    fn an_empty_chain_mixes_exactly_as_the_plain_formula() {
        let mut e = engine();
        e.configure_track(0, Instrument::preset("Lead"), 0.7, 0.3, false, false);
        e.set_effects(0, &EffectChain::EMPTY);
        constant(&mut e, 0, 512, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let gain = 0.7 * Instrument::preset("Lead").gain;
        let pan = 0.3f32;
        let expected_l = (0.5 * gain * (0.5 * (1.0 - pan)) * e.master).clamp(-1.0, 1.0);
        let expected_r = (0.5 * gain * (0.5 * (1.0 + pan)) * e.master).clamp(-1.0, 1.0);
        let mut peaks = [0.0; MAX_TRACKS];
        for _ in 0..256 {
            assert_eq!(e.process_frame(&mut peaks), (expected_l, expected_r));
        }
        assert_eq!(peaks[0], 0.5);
    }

    #[test]
    fn the_meter_reads_after_the_chain_and_before_the_fader() {
        let mut e = engine();
        e.configure_track(0, Instrument::preset("Lead"), 0.1, 0.0, false, false);
        let mut quiet = Effect::new(EffectKind::Saturator);
        quiet.params = [0.0, -20.0, 1.0, 0.0, 0.0];
        e.set_effects(0, &chain(&[quiet]));
        constant(&mut e, 0, 4_096, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let peaks = run(&mut e, 2_048);
        let expected = 0.5f32.tanh() * 0.1;
        assert!((peaks[0] - expected).abs() < 1e-3, "{}", peaks[0]);
    }

    #[test]
    fn a_delay_tail_outlives_the_last_voice() {
        let mut e = engine();
        e.set_tempo(120.0);
        let mut delay = Effect::new(EffectKind::Delay);
        delay.params = [0.5, 0.0, 0.5, 0.0, 0.0];
        e.set_effects(0, &chain(&[delay]));
        constant(&mut e, 0, 64, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        let mut peaks = [0.0; MAX_TRACKS];
        let mut echo = 0.0f32;
        for frame in 0..30_000 {
            let (l, _) = e.process_frame(&mut peaks);
            if frame > 1_000 {
                echo = echo.max(l.abs());
            }
        }
        assert!(!e.is_active(), "the sample voice ended long ago");
        assert!(echo > 0.01, "the echo still sounded: {echo}");
        run(&mut e, 44_100);
        assert!(!e.is_ringing(), "the tail ended");
        assert_eq!(e.process_frame(&mut peaks), (0.0, 0.0));
    }

    #[test]
    fn a_muted_track_keeps_running_its_chain() {
        let mut e = engine();
        e.configure_track(0, Instrument::preset("Lead"), 1.0, 0.0, true, false);
        e.set_effects(0, &chain(&[Effect::new(EffectKind::Reverb)]));
        constant(&mut e, 0, 64, 0.5);
        assert!(e.play_sample(0, EVENT, 0, SampleCue::START, 1.0, 0));
        run(&mut e, 2_000);
        assert!(e.is_ringing());
        e.configure_track(0, Instrument::preset("Lead"), 1.0, 0.0, false, false);
        let mut peaks = [0.0; MAX_TRACKS];
        let tail = (0..4_000)
            .map(|_| e.process_frame(&mut peaks).0.abs())
            .fold(0.0, f32::max);
        assert!(tail > 1e-4, "the tail built up while muted: {tail}");
    }
}
