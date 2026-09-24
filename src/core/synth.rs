//! A pre-allocated, allocation-free polyphonic synthesizer.
//!
//! Voices are stored in fixed-size arrays so the audio callback only ever
//! mutates existing memory. Envelope increments are derived from the
//! [`Instrument`] and the sample rate whenever an instrument changes.

use crate::core::{MAX_TRACKS, MAX_VOICES};
use crate::models::{Instrument, Waveform};
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

/// One track's voices and mixer state.
#[derive(Clone, Copy, Debug)]
struct TrackVoices {
    voices: [Voice; MAX_VOICES],
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

/// A fixed-array polyphonic synthesizer for up to [`MAX_TRACKS`] tracks.
pub struct SynthEngine {
    sample_rate: f32,
    tracks: [TrackVoices; MAX_TRACKS],
    track_count: usize,
    active_voices: usize,
    any_solo: bool,
    master: f32,
    rng: u32,
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

    fn update_solo(&mut self) {
        self.any_solo = self.tracks[..self.track_count].iter().any(|t| t.solo);
    }

    fn recompute_active_voices(&mut self) {
        self.active_voices = self.tracks[..self.track_count]
            .iter()
            .flat_map(|t| t.voices.iter())
            .filter(|v| !v.is_idle())
            .count();
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
    pub fn note_off(&mut self, track: usize, pitch: u8) {
        if track >= self.track_count {
            return;
        }
        for voice in &mut self.tracks[track].voices {
            if !voice.is_idle() && voice.note == pitch {
                voice.gate_left = 0;
            }
        }
    }

    /// Release every voice on every track.
    pub fn all_notes_off(&mut self) {
        for track in &mut self.tracks[..self.track_count] {
            for voice in &mut track.voices {
                if !voice.is_idle() {
                    voice.gate_left = 0;
                }
            }
        }
    }

    /// Whether any voice is currently producing sound.
    pub fn is_active(&self) -> bool {
        self.active_voices > 0
    }

    /// Render one stereo frame, accumulating per-track peaks into `peaks`.
    ///
    /// Returns `(left, right)` already clamped to `[-1, 1]`.
    pub fn process_frame(&mut self, peaks: &mut [f32; MAX_TRACKS]) -> (f32, f32) {
        if self.active_voices == 0 {
            return (0.0, 0.0);
        }

        let SynthEngine {
            tracks,
            track_count,
            any_solo,
            master,
            rng,
            active_voices,
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

            let magnitude = mono.abs();
            if magnitude > peaks[ti] {
                peaks[ti] = magnitude;
            }

            let audible = !track.muted && (!*any_solo || track.solo);
            if audible {
                let gain = track.volume * instrument.gain;
                let pan = track.pan;
                sum_l += mono * gain * (0.5 * (1.0 - pan));
                sum_r += mono * gain * (0.5 * (1.0 + pan));
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
    use crate::models::Instrument;

    fn engine() -> SynthEngine {
        let mut e = SynthEngine::new(44_100);
        e.set_track_count(1);
        e.configure_track(0, Instrument::preset("Lead"), 1.0, 0.0, false, false);
        e
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
}
