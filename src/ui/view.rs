//! The logical screen model.
//!
//! [`UiState`] is built once per frame from the song and the audio engine's
//! shared state. It holds no references into `core`, so both the ratatui
//! renderer and the [`View`] plain-text renderer can consume it.

use crate::models::TrackKind;
use crate::ui::layout::Pane;

/// Beats per bar assumed by the position readout.
pub const BEATS_PER_BAR: u32 = 4;

/// Session cursor: which block of which track is selected. A block index equal
/// to the track's block count means the `+` slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub block: usize,
    pub track: usize,
}

/// A track, reduced to what the UI needs.
#[derive(Clone, Debug)]
pub struct TrackView {
    pub name: String,
    pub kind: TrackKind,
    pub preset: String,
    pub waveform: String,
    pub octave: i32,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    /// Smoothed meter level.
    pub level: f32,
    /// Block labels, in column order.
    pub blocks: Vec<String>,
    /// Whether the cursor sits on this track's `+` slot.
    pub add_slot: bool,
    /// Index into the renderer's track palette.
    pub color_index: u8,
}

impl TrackView {
    /// How many blocks the track holds.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Everything worth drawing this frame.
#[derive(Clone, Debug)]
pub struct UiState {
    pub bpm: f32,
    pub playing: bool,
    pub looping: bool,
    /// Transport position in beats.
    pub transport_position_beats: f32,
    pub cursor: Cursor,
    pub focus: Pane,
    pub octave: i32,
    pub tracks: Vec<TrackView>,
    pub status: String,
    pub backend: String,
    /// Why the engine chose its backend.
    pub backend_note: String,
    pub sample_rate: u32,
    pub show_help: bool,
}

impl UiState {
    /// The track under the cursor, if any.
    pub fn selected(&self) -> Option<&TrackView> {
        self.tracks.get(self.cursor.track)
    }
}

/// Bar and beat (both one-based) for a position in beats.
pub fn bar_beat(beats: f32) -> (u32, u32) {
    let beats = beats.max(0.0);
    let whole = beats as u32;
    (whole / BEATS_PER_BAR + 1, whole % BEATS_PER_BAR + 1)
}

/// Render a state to a plain string. Implemented by [`UiState`] so the app can
/// run headless (no terminal, no ratatui).
pub trait View {
    /// Produce a textual snapshot.
    fn render(&self) -> String;
}

pub(crate) fn meter_bar(level: f32, width: usize) -> String {
    let filled = ((level.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    let mut bar = String::with_capacity(width);
    for i in 0..width {
        bar.push(if i < filled { '#' } else { '-' });
    }
    bar
}

impl View for UiState {
    fn render(&self) -> String {
        let (bar, beat) = bar_beat(self.transport_position_beats);
        let mut out = String::new();
        out.push_str(&format!(
            "zboub69  bpm {:.0}  {}  loop {}  bar {bar}.{beat}  backend {} @ {} Hz\n",
            self.bpm,
            if self.playing { "PLAYING" } else { "stopped" },
            if self.looping { "on" } else { "off" },
            self.backend,
            self.sample_rate,
        ));
        out.push_str(&format!(
            "focus {:?}  keyboard octave {:+}\n",
            self.focus, self.octave
        ));
        out.push_str(&format!("{}\n\n", self.backend_note));

        out.push_str("tracks\n");
        for (index, track) in self.tracks.iter().enumerate() {
            let selected = index == self.cursor.track;
            out.push_str(&format!(
                "  {} {:>2} {:<8} {:<5} ",
                if selected { '>' } else { ' ' },
                index + 1,
                track.name,
                track.kind.name(),
            ));
            for (block_index, name) in track.blocks.iter().enumerate() {
                let on_cursor = selected && block_index == self.cursor.block;
                let cell = if on_cursor {
                    format!("[{name}]")
                } else {
                    format!(" {name} ")
                };
                out.push_str(&cell);
            }
            out.push_str(if track.add_slot { "[+]" } else { " + " });
            out.push_str(&format!(
                " {} vol {:.2}\n",
                meter_bar(track.level, 10),
                track.volume
            ));
        }

        out.push_str(&format!("\n{}\n", self.status));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(name: &str, blocks: &[&str], add_slot: bool) -> TrackView {
        TrackView {
            name: name.into(),
            kind: TrackKind::Midi,
            preset: "Lead".into(),
            waveform: "Square".into(),
            octave: 0,
            attack: 0.01,
            decay: 0.12,
            sustain: 0.6,
            release: 0.15,
            volume: 0.8,
            pan: 0.0,
            muted: false,
            solo: false,
            level: 0.5,
            blocks: blocks.iter().map(|b| (*b).to_string()).collect(),
            add_slot,
            color_index: 0,
        }
    }

    fn state() -> UiState {
        UiState {
            bpm: 120.0,
            playing: true,
            looping: true,
            transport_position_beats: 5.0,
            cursor: Cursor { track: 0, block: 1 },
            focus: Pane::Tracks,
            octave: 0,
            tracks: vec![
                track("Midi 1", &["Verse", "Chorus"], false),
                track("Audio 1", &[], true),
            ],
            status: "ready".into(),
            backend: "silent".into(),
            backend_note: "silent: no device".into(),
            sample_rate: 44_100,
            show_help: false,
        }
    }

    #[test]
    fn plain_render_contains_the_essentials() {
        let text = state().render();
        assert!(text.contains("zboub69"));
        assert!(text.contains("PLAYING"));
        assert!(text.contains("Midi 1"));
        assert!(text.contains("tracks"));
        assert!(text.contains("ready"));
        assert!(text.contains("bar 2.2"));
    }

    #[test]
    fn empty_tracks_still_render_a_plus_slot() {
        let state = UiState {
            tracks: vec![track("Midi 1", &[], true)],
            ..state()
        };
        let text = state.render();
        assert!(text.contains("[+]"), "the add slot should be visible");
    }

    #[test]
    fn cursor_block_is_marked() {
        let text = state().render();
        assert!(
            text.contains("[Chorus]"),
            "cursor block should be bracketed"
        );
    }

    #[test]
    fn selected_track_follows_the_cursor() {
        assert_eq!(state().selected().map(|t| t.name.as_str()), Some("Midi 1"));
    }
}
