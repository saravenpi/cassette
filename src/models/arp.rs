//! The arpeggiator: a per-track MIDI device that turns held chords into
//! patterns, modeled on Ableton Live's Arpeggiator.
//!
//! It is non-destructive. The blocks keep the notes the user drew, and
//! [`arpeggiate`] rewrites a lane's flattened notes on the control thread just
//! before they are mirrored to the audio thread, so playback and export hear
//! the same pattern.
//!
//! # Timing
//!
//! Steps sit on a grid of [`Arp::rate`] beats counted from the top of the song.
//! Step `k` falls at `k * rate`; when `k` is odd it is pushed later by
//! `swing * rate`, so a swing of `1/3` gives a triplet shuffle. A step sounds
//! when at least one note is held at its time: a note is held from its start
//! up to, not including, its end. A chord is a run of notes that overlap or
//! touch; the pattern restarts at the first step of each chord, and every step
//! note is cut at the chord's end.
//!
//! # Pattern
//!
//! The held notes, sorted by pitch (or by start for [`ArpStyle::PlayOrder`]),
//! give one pass of the style. The pass repeats `steps` more times, each repeat
//! transposed by another `distance` semitones, then the whole run wraps. When
//! the chord changes mid-pattern the step counter keeps counting and indexes
//! the new run, so the pattern carries on instead of restarting.

use serde::{Deserialize, Serialize};

use super::Note;

/// Order the held notes play in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArpStyle {
    /// Lowest to highest.
    #[default]
    Up,
    /// Highest to lowest.
    Down,
    /// Up then down, without repeating the top and bottom notes.
    UpDown,
    /// Down then up, without repeating the top and bottom notes.
    DownUp,
    /// From the outside in: lowest, highest, second lowest, second highest.
    Converge,
    /// From the inside out: the reverse of [`ArpStyle::Converge`].
    Diverge,
    /// In the order the notes started, lower pitch first on a tie.
    PlayOrder,
    /// A pick from the run at every step, the same for the same seed.
    Random,
}

/// Length of one step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArpRate {
    #[serde(rename = "1/1")]
    Whole,
    #[serde(rename = "1/2")]
    Half,
    #[serde(rename = "1/4")]
    Quarter,
    #[serde(rename = "1/8")]
    Eighth,
    #[default]
    #[serde(rename = "1/16")]
    Sixteenth,
    #[serde(rename = "1/32")]
    ThirtySecond,
    #[serde(rename = "1/4T")]
    QuarterTriplet,
    #[serde(rename = "1/8T")]
    EighthTriplet,
    #[serde(rename = "1/16T")]
    SixteenthTriplet,
}

impl ArpRate {
    /// Length of one step, in beats.
    pub const fn beats(self) -> f64 {
        match self {
            ArpRate::Whole => 4.0,
            ArpRate::Half => 2.0,
            ArpRate::Quarter => 1.0,
            ArpRate::Eighth => 0.5,
            ArpRate::Sixteenth => 0.25,
            ArpRate::ThirtySecond => 0.125,
            ArpRate::QuarterTriplet => 2.0 / 3.0,
            ArpRate::EighthTriplet => 1.0 / 3.0,
            ArpRate::SixteenthTriplet => 1.0 / 6.0,
        }
    }
}

/// A track's arpeggiator settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Arp {
    pub enabled: bool,
    pub style: ArpStyle,
    pub rate: ArpRate,
    /// Step note length as a share of the step, `[0.01, 2]`; above one the
    /// notes overlap.
    pub gate: f32,
    /// Semitones each repeat of the pattern moves, `[-24, 24]`.
    pub distance: i8,
    /// Transposed repeats after the first pass, `[0, 8]`.
    pub steps: u8,
    /// Share of a step every odd step is pushed later by, `[0, 0.75]`.
    pub swing: f32,
    /// Beats from a chord's start for the velocity to reach
    /// [`Arp::velocity_target`], `[0, 16]`; zero keeps the drawn velocities.
    pub velocity_decay: f32,
    /// Velocity the decay slides toward.
    pub velocity_target: u8,
    /// Seed of the [`ArpStyle::Random`] pattern.
    pub seed: u32,
}

impl Default for Arp {
    fn default() -> Self {
        Self {
            enabled: false,
            style: ArpStyle::Up,
            rate: ArpRate::Sixteenth,
            gate: 0.8,
            distance: 12,
            steps: 0,
            swing: 0.0,
            velocity_decay: 0.0,
            velocity_target: 0,
            seed: 0,
        }
    }
}

impl Arp {
    pub const MIN_GATE: f32 = 0.01;
    pub const MAX_GATE: f32 = 2.0;
    pub const MAX_DISTANCE: i8 = 24;
    pub const MAX_STEPS: u8 = 8;
    pub const MAX_SWING: f32 = 0.75;
    pub const MAX_VELOCITY_DECAY: f32 = 16.0;

    /// Bring every setting inside its range, a number that is not finite
    /// falling back to its default. Returns whether anything changed.
    pub fn sanitize(&mut self) -> bool {
        let before = *self;
        let fallback = Self::default();
        self.gate = finite_or(self.gate, fallback.gate).clamp(Self::MIN_GATE, Self::MAX_GATE);
        self.distance = self.distance.clamp(-Self::MAX_DISTANCE, Self::MAX_DISTANCE);
        self.steps = self.steps.min(Self::MAX_STEPS);
        self.swing = finite_or(self.swing, fallback.swing).clamp(0.0, Self::MAX_SWING);
        self.velocity_decay = finite_or(self.velocity_decay, fallback.velocity_decay)
            .clamp(0.0, Self::MAX_VELOCITY_DECAY);
        self.velocity_target = self.velocity_target.min(Note::MAX_VELOCITY);
        before != *self
    }

    /// Time of step `k`, in beats from the top of the song.
    fn step_time(&self, k: u64) -> f64 {
        let rate = self.rate.beats();
        let swing = if k % 2 == 1 {
            f64::from(self.swing) * rate
        } else {
            0.0
        };
        k as f64 * rate + swing
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// Slack, in beats, for comparing a step time with a drawn note's edges.
const EPSILON: f64 = 1e-4;

/// Rewrite notes through the arpeggiator.
///
/// `notes` hold absolute starts, in beats from the top of the song, and are
/// already cut to their blocks. A disabled arpeggiator returns them as given.
/// Otherwise the result is the step notes, sorted by start; see the module
/// documentation for how they are placed.
pub fn arpeggiate(notes: &[Note], arp: &Arp) -> Vec<Note> {
    if !arp.enabled {
        return notes.to_vec();
    }
    let mut sorted: Vec<Note> = notes
        .iter()
        .copied()
        .filter(|note| note.is_finite() && note.length_beats > 0.0)
        .collect();
    sorted.sort_by(|a, b| {
        a.start_beats
            .total_cmp(&b.start_beats)
            .then(a.pitch.cmp(&b.pitch))
    });
    let mut out = Vec::new();
    for chord in chords(&sorted) {
        play_chord(chord, arp, &mut out);
    }
    out.sort_by(|a, b| a.start_beats.total_cmp(&b.start_beats));
    out
}

fn end_of(note: &Note) -> f64 {
    f64::from(note.start_beats) + f64::from(note.length_beats)
}

/// Split notes sorted by start into runs that overlap or touch.
fn chords(sorted: &[Note]) -> Vec<&[Note]> {
    let mut runs = Vec::new();
    let mut first = 0;
    let mut end = f64::NEG_INFINITY;
    for (index, note) in sorted.iter().enumerate() {
        if index > first && f64::from(note.start_beats) > end + EPSILON {
            runs.push(&sorted[first..index]);
            first = index;
            end = f64::NEG_INFINITY;
        }
        end = end.max(end_of(note));
    }
    if first < sorted.len() {
        runs.push(&sorted[first..]);
    }
    runs
}

/// Append the step notes of one chord.
fn play_chord(chord: &[Note], arp: &Arp, out: &mut Vec<Note>) {
    let start = f64::from(chord[0].start_beats);
    let end = chord.iter().map(end_of).fold(f64::NEG_INFINITY, f64::max);
    let rate = arp.rate.beats();
    let length = rate * f64::from(arp.gate);
    let mut played = 0usize;
    let mut k = ((start / rate).floor() as u64).saturating_sub(1);
    loop {
        let at = arp.step_time(k);
        if at >= end - EPSILON {
            break;
        }
        if at >= start - EPSILON {
            let held: Vec<Note> = chord
                .iter()
                .filter(|note| {
                    f64::from(note.start_beats) <= at + EPSILON && end_of(note) > at + EPSILON
                })
                .copied()
                .collect();
            let run = sequence(&held, arp);
            if !run.is_empty() {
                let index = match arp.style {
                    ArpStyle::Random => (mix(arp.seed, k) % run.len() as u64) as usize,
                    _ => played % run.len(),
                };
                let (pitch, velocity) = run[index];
                out.push(Note {
                    pitch,
                    start_beats: at as f32,
                    length_beats: length.min(end - at) as f32,
                    velocity: shaped_velocity(velocity, at - start, arp),
                });
                played += 1;
            }
        }
        k += 1;
    }
}

/// The velocity of a step `since` beats into its chord.
fn shaped_velocity(velocity: u8, since: f64, arp: &Arp) -> u8 {
    if arp.velocity_decay <= 0.0 {
        return velocity;
    }
    let share = (since / f64::from(arp.velocity_decay)).clamp(0.0, 1.0);
    let from = f64::from(velocity);
    let to = f64::from(arp.velocity_target);
    (from + (to - from) * share)
        .round()
        .clamp(f64::from(Note::MIN_VELOCITY), f64::from(Note::MAX_VELOCITY)) as u8
}

/// The full run for the held notes: one pass of the style, then `steps`
/// transposed repeats, as `(pitch, velocity)` pairs.
fn sequence(held: &[Note], arp: &Arp) -> Vec<(u8, u8)> {
    let mut pool: Vec<Note> = Vec::with_capacity(held.len());
    for note in held {
        if !pool.iter().any(|kept| kept.pitch == note.pitch) {
            pool.push(*note);
        }
    }
    if arp.style != ArpStyle::PlayOrder {
        pool.sort_by_key(|note| note.pitch);
    }
    let pass: Vec<Note> = order(pool.len(), arp.style)
        .into_iter()
        .map(|index| pool[index])
        .collect();
    (0..=i32::from(arp.steps))
        .flat_map(|repeat| {
            pass.iter().map(move |note| {
                let pitch = i32::from(note.pitch) + repeat * i32::from(arp.distance);
                (
                    pitch.clamp(0, i32::from(Note::MAX_PITCH)) as u8,
                    note.velocity,
                )
            })
        })
        .collect()
}

/// Indices into `n` notes sorted by pitch, in the order one pass plays them.
fn order(n: usize, style: ArpStyle) -> Vec<usize> {
    let up: Vec<usize> = (0..n).collect();
    let down: Vec<usize> = (0..n).rev().collect();
    let inner = |run: &[usize]| run[1..n - 1].to_vec();
    match style {
        ArpStyle::Up | ArpStyle::PlayOrder | ArpStyle::Random => up,
        ArpStyle::Down => down,
        ArpStyle::UpDown if n > 2 => [up.clone(), inner(&down)].concat(),
        ArpStyle::DownUp if n > 2 => [down.clone(), inner(&up)].concat(),
        ArpStyle::UpDown => up,
        ArpStyle::DownUp => down,
        ArpStyle::Converge => converge(n),
        ArpStyle::Diverge => converge(n).into_iter().rev().collect(),
    }
}

fn converge(n: usize) -> Vec<usize> {
    (0..n)
        .map(|i| if i % 2 == 0 { i / 2 } else { n - 1 - i / 2 })
        .collect()
}

/// A well-mixed hash of the seed and a step number, splitmix64.
fn mix(seed: u32, step: u64) -> u64 {
    let mut z = (u64::from(seed) << 32 ^ step).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(pitch: u8, start_beats: f32, length_beats: f32) -> Note {
        Note {
            pitch,
            start_beats,
            length_beats,
            velocity: 100,
        }
    }

    fn arp(style: ArpStyle) -> Arp {
        Arp {
            enabled: true,
            style,
            rate: ArpRate::Quarter,
            gate: 0.5,
            ..Arp::default()
        }
    }

    fn chord(length: f32) -> Vec<Note> {
        vec![
            note(64, 0.0, length),
            note(60, 0.0, length),
            note(67, 0.0, length),
            note(72, 0.0, length),
        ]
    }

    fn pitches(notes: &[Note]) -> Vec<u8> {
        notes.iter().map(|note| note.pitch).collect()
    }

    #[test]
    fn every_style_plays_its_order() {
        let cases = [
            (ArpStyle::Up, vec![60, 64, 67, 72, 60, 64, 67, 72]),
            (ArpStyle::Down, vec![72, 67, 64, 60, 72, 67, 64, 60]),
            (ArpStyle::UpDown, vec![60, 64, 67, 72, 67, 64, 60, 64]),
            (ArpStyle::DownUp, vec![72, 67, 64, 60, 64, 67, 72, 67]),
            (ArpStyle::Converge, vec![60, 72, 64, 67, 60, 72, 64, 67]),
            (ArpStyle::Diverge, vec![67, 64, 72, 60, 67, 64, 72, 60]),
            (ArpStyle::PlayOrder, vec![60, 64, 67, 72, 60, 64, 67, 72]),
        ];
        for (style, expected) in cases {
            assert_eq!(
                pitches(&arpeggiate(&chord(8.0), &arp(style))),
                expected,
                "{style:?}"
            );
        }
    }

    #[test]
    fn play_order_follows_the_starts() {
        let notes = vec![note(72, 0.0, 4.0), note(60, 1.0, 3.0), note(67, 2.0, 2.0)];
        assert_eq!(
            pitches(&arpeggiate(&notes, &arp(ArpStyle::PlayOrder))),
            vec![72, 60, 67, 72],
            "the counter keeps going as notes join"
        );
    }

    #[test]
    fn random_is_deterministic_and_stays_in_the_chord() {
        let settings = Arp {
            seed: 7,
            ..arp(ArpStyle::Random)
        };
        let first = arpeggiate(&chord(32.0), &settings);
        assert_eq!(first, arpeggiate(&chord(32.0), &settings));
        assert_eq!(first.len(), 32);
        assert!(first.iter().all(|n| [60, 64, 67, 72].contains(&n.pitch)));
        let other = arpeggiate(
            &chord(32.0),
            &Arp {
                seed: 8,
                ..settings
            },
        );
        assert_ne!(pitches(&first), pitches(&other));
    }

    #[test]
    fn steps_sit_on_the_grid_with_the_gate_length() {
        let played = arpeggiate(&chord(2.0), &arp(ArpStyle::Up));
        let times: Vec<(f32, f32)> = played
            .iter()
            .map(|n| (n.start_beats, n.length_beats))
            .collect();
        assert_eq!(times, vec![(0.0, 0.5), (1.0, 0.5)]);
    }

    #[test]
    fn a_gate_past_one_overlaps_and_stops_at_the_chord_end() {
        let settings = Arp {
            gate: 1.5,
            ..arp(ArpStyle::Up)
        };
        let played = arpeggiate(&chord(3.0), &settings);
        let lengths: Vec<f32> = played.iter().map(|n| n.length_beats).collect();
        assert_eq!(lengths, vec![1.5, 1.5, 1.0]);
        assert!(played[0].start_beats + played[0].length_beats > played[1].start_beats);
    }

    #[test]
    fn distance_and_steps_transpose_each_repeat_then_wrap() {
        let settings = Arp {
            distance: 12,
            steps: 2,
            ..arp(ArpStyle::Up)
        };
        let notes = vec![note(60, 0.0, 7.0), note(64, 0.0, 7.0)];
        assert_eq!(
            pitches(&arpeggiate(&notes, &settings)),
            vec![60, 64, 72, 76, 84, 88, 60]
        );
        let down = Arp {
            distance: -24,
            steps: 3,
            ..settings
        };
        let low = vec![note(10, 0.0, 4.0)];
        assert_eq!(
            pitches(&arpeggiate(&low, &down)),
            vec![10, 0, 0, 0],
            "pitches clamp at 0"
        );
    }

    #[test]
    fn swing_pushes_odd_steps_later() {
        let settings = Arp {
            swing: 0.5,
            ..arp(ArpStyle::Up)
        };
        let starts: Vec<f32> = arpeggiate(&chord(4.0), &settings)
            .iter()
            .map(|n| n.start_beats)
            .collect();
        assert_eq!(starts, vec![0.0, 1.5, 2.0, 3.5]);
    }

    #[test]
    fn velocity_slides_toward_the_target_from_the_chord_start() {
        let settings = Arp {
            velocity_decay: 4.0,
            velocity_target: 20,
            ..arp(ArpStyle::Up)
        };
        let notes: Vec<Note> = [0.0, 8.0]
            .into_iter()
            .map(|start| note(60, start, 6.0))
            .collect();
        let velocities: Vec<u8> = arpeggiate(&notes, &settings)
            .iter()
            .map(|n| n.velocity)
            .collect();
        assert_eq!(
            velocities,
            vec![100, 80, 60, 40, 20, 20, 100, 80, 60, 40, 20, 20],
            "the slope restarts with each chord"
        );
    }

    #[test]
    fn a_chord_change_mid_pattern_keeps_the_counter() {
        let notes = vec![
            note(60, 0.0, 2.0),
            note(64, 0.0, 2.0),
            note(67, 0.0, 2.0),
            note(62, 2.0, 2.0),
            note(65, 2.0, 2.0),
        ];
        assert_eq!(
            pitches(&arpeggiate(&notes, &arp(ArpStyle::Up))),
            vec![60, 64, 62, 65],
            "step 2 plays index 0 of the new pair, step 3 index 1"
        );
    }

    #[test]
    fn steps_wait_for_the_grid_and_skip_gaps() {
        let notes = vec![note(60, 0.5, 1.0), note(62, 3.2, 0.5)];
        let played = arpeggiate(&notes, &arp(ArpStyle::Up));
        assert_eq!(played.len(), 1);
        assert_eq!(played[0].start_beats, 1.0);
        assert_eq!(played[0].length_beats, 0.5);
    }

    #[test]
    fn triplet_rates_land_on_drawn_triplets() {
        let settings = Arp {
            rate: ArpRate::EighthTriplet,
            ..arp(ArpStyle::Up)
        };
        let notes = vec![note(60, 1.0 / 3.0, 1.0 / 3.0)];
        assert_eq!(arpeggiate(&notes, &settings).len(), 1);
    }

    #[test]
    fn disabled_or_empty_passes_through() {
        let notes = vec![note(67, 1.0, 1.0), note(60, 0.0, 0.3)];
        assert_eq!(arpeggiate(&notes, &Arp::default()), notes);
        assert!(arpeggiate(&[], &arp(ArpStyle::Up)).is_empty());
    }

    #[test]
    fn sanitize_clamps_every_setting() {
        let mut wild = Arp {
            gate: f32::NAN,
            distance: 100,
            steps: 40,
            swing: 3.0,
            velocity_decay: -1.0,
            velocity_target: 200,
            ..Arp::default()
        };
        assert!(wild.sanitize());
        assert_eq!(wild.gate, 0.8);
        assert_eq!(wild.distance, 24);
        assert_eq!(wild.steps, 8);
        assert_eq!(wild.swing, 0.75);
        assert_eq!(wild.velocity_decay, 0.0);
        assert_eq!(wild.velocity_target, 127);
        assert!(!wild.sanitize());
    }

    #[test]
    fn settings_serialize_with_readable_names() {
        let json = serde_json::to_string(&Arp {
            style: ArpStyle::UpDown,
            rate: ArpRate::EighthTriplet,
            ..Arp::default()
        })
        .expect("serialize");
        assert!(json.contains("\"style\":\"upDown\""));
        assert!(json.contains("\"rate\":\"1/8T\""));
        assert!(json.contains("\"velocityDecay\":0.0"));
        let partial: Arp = serde_json::from_str("{\"enabled\":true}").expect("partial");
        assert_eq!(
            partial,
            Arp {
                enabled: true,
                ..Arp::default()
            }
        );
    }
}
