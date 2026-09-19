//! The logical screen model.
//!
//! [`UiState`] is built once per frame from the song and the audio engine's
//! shared state. It holds no references into `core`, so both the ratatui
//! renderer and the [`View`] plain-text renderer can consume it.

use crate::ui::layout::Pane;

/// Pattern cursor: which step of which track is selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub step: usize,
    pub track: usize,
}

/// A step, reduced to what the UI needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepView {
    pub active: bool,
    pub pitch: u8,
}

/// A track, reduced to what the UI needs.
#[derive(Clone, Debug)]
pub struct TrackView {
    pub name: String,
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
    /// Held peak.
    pub peak: f32,
    pub steps: Vec<StepView>,
    /// Index into the renderer's track palette.
    pub color_index: u8,
}

impl TrackView {
    /// Pitch of a step, or `None` when out of range.
    pub fn pitch_at(&self, step: usize) -> Option<u8> {
        self.steps.get(step).map(|s| s.pitch)
    }
}

/// Everything worth drawing this frame.
#[derive(Clone, Debug)]
pub struct UiState {
    pub bpm: f32,
    pub playing: bool,
    pub recording: bool,
    pub looping: bool,
    pub position_step: usize,
    pub length_steps: usize,
    pub cursor: Cursor,
    pub focus: Pane,
    pub octave: i32,
    pub tracks: Vec<TrackView>,
    pub status: String,
    pub backend: String,
    pub sample_rate: u32,
    pub show_help: bool,
}

impl UiState {
    /// The track under the cursor, if any.
    pub fn selected(&self) -> Option<&TrackView> {
        self.tracks.get(self.cursor.track)
    }
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
        let mut out = String::new();
        out.push_str(&format!(
            "zboub69  bpm {:.0}  {}  loop {}  step {}/{}  backend {} @ {} Hz\n",
            self.bpm,
            if self.playing { "PLAYING" } else { "stopped" },
            if self.looping { "on" } else { "off" },
            self.position_step + 1,
            self.length_steps,
            self.backend,
            self.sample_rate,
        ));
        if self.recording {
            out.push_str("recording armed\n");
        }
        out.push_str(&format!(
            "focus {:?}  keyboard octave {:+}\n\n",
            self.focus, self.octave
        ));

        out.push_str("tracks\n");
        for (index, track) in self.tracks.iter().enumerate() {
            let flags = format!(
                "{}{}",
                if track.muted { 'M' } else { '-' },
                if track.solo { 'S' } else { '-' }
            );
            out.push_str(&format!(
                "  [{:>2}] {:<8} {:<6} {:<9} vol {:.2} pan {:+.2} {} {}\n",
                index + 1,
                track.name,
                track.preset,
                track.waveform,
                track.volume,
                track.pan,
                flags,
                meter_bar(track.level, 10),
            ));
        }

        out.push_str("\npattern\n        ");
        for step in 0..self.length_steps {
            out.push_str(if step % 4 == 0 { "| " } else { "  " });
        }
        out.push('\n');

        for (track_index, track) in self.tracks.iter().enumerate() {
            out.push_str(&format!("{:<8}", track.name));
            for (step_index, step) in track.steps.iter().enumerate() {
                let glyph = if step.active { '●' } else { '·' };
                let is_cursor = track_index == self.cursor.track && step_index == self.cursor.step;
                let is_playing = self.playing && step_index == self.position_step;
                let cell = if is_cursor {
                    format!("[{glyph}]")
                } else if is_playing {
                    format!("<{glyph}>")
                } else {
                    format!(" {glyph} ")
                };
                out.push_str(&cell);
            }
            out.push('\n');
        }

        out.push_str(&format!("\n{}\n", self.status));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> UiState {
        UiState {
            bpm: 120.0,
            playing: true,
            recording: false,
            looping: true,
            position_step: 4,
            length_steps: 8,
            cursor: Cursor { step: 2, track: 0 },
            focus: Pane::Pattern,
            octave: 0,
            tracks: vec![TrackView {
                name: "Kick".into(),
                preset: "Kick".into(),
                waveform: "Sine".into(),
                octave: -1,
                attack: 0.0,
                decay: 0.2,
                sustain: 0.0,
                release: 0.1,
                volume: 0.8,
                pan: 0.0,
                muted: false,
                solo: false,
                level: 0.5,
                peak: 0.9,
                steps: vec![
                    StepView {
                        active: true,
                        pitch: 36,
                    };
                    8
                ],
                color_index: 0,
            }],
            status: "ready".into(),
            backend: "silent".into(),
            sample_rate: 44_100,
            show_help: false,
        }
    }

    #[test]
    fn plain_render_contains_the_essentials() {
        let text = state().render();
        assert!(text.contains("zboub69"));
        assert!(text.contains("PLAYING"));
        assert!(text.contains("Kick"));
        assert!(text.contains("pattern"));
        assert!(text.contains("ready"));
    }

    #[test]
    fn cursor_cell_is_marked() {
        let text = state().render();
        assert!(text.contains('['), "cursor step should be bracketed");
    }

    #[test]
    fn selected_track_follows_the_cursor() {
        let state = state();
        assert_eq!(state.selected().map(|t| t.name.as_str()), Some("Kick"));
    }
}
