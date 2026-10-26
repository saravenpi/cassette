//! The transport, the block-name prompt, the notes the keyboard is holding and
//! the MIDI take they are recorded into.

use std::sync::atomic::Ordering;

use super::App;
use super::BEATS_PER_BAR;
use super::notes::grid_name;
use crate::core::render::RtCommand;
use crate::models::{LoopRegion, MAX_NOTES_PER_BLOCK, Note, TrackKind};
use crate::ui::view::Cursor;
use crate::utils::time::Tempo;

/// Longest block name the prompt accepts, in characters.
const MAX_NAME_LEN: usize = 32;
/// Count-in lengths the transport menu offers, in bars.
const COUNT_IN_BARS: [u8; 4] = [0, 1, 2, 4];
/// Finest grid record quantize snaps to, in beats.
const MIN_RECORD_GRID: f32 = 1.0 / 16.0;
/// Coarsest grid record quantize snaps to, in beats.
const MAX_RECORD_GRID: f32 = 4.0;

/// A note waiting to be released.
pub(crate) struct HeldNote {
    track: u8,
    pitch: u8,
    velocity: u8,
    /// Beat the note went down on, when a take is recording it.
    recorded_from: Option<f32>,
}

/// The block a take grows, named by its lane and its start so a re-sort of the
/// lane does not lose it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Take {
    track: usize,
    start_beats: f32,
}

impl App {
    /// Start playing, or stop if already playing.
    pub(crate) fn toggle_play(&mut self) {
        if self.engine.is_playing() {
            self.stop();
        } else {
            self.send(RtCommand::Seek {
                beats: self.marker_beats,
            });
            self.send(RtCommand::Play);
            self.status = "playing".into();
        }
    }

    /// Move the playhead and the marker that play starts from and stop returns to.
    pub(crate) fn seek(&mut self, beats: f32) {
        self.marker_beats = beats.max(0.0);
        self.send(RtCommand::Seek {
            beats: self.marker_beats,
        });
        self.status = format!("seek {:.2} beats", self.marker_beats);
    }

    /// Stop and return to the marker. Stopping while stopped returns to the top.
    pub(crate) fn stop(&mut self) {
        self.end_take();
        if !self.engine.is_playing() {
            self.marker_beats = 0.0;
        }
        self.send(RtCommand::Stop);
        self.send(RtCommand::Seek {
            beats: self.marker_beats,
        });
        self.status = "stopped".into();
    }

    /// Arm or disarm MIDI recording. Arming while stopped starts the transport
    /// from the marker, after the count-in when one is set.
    pub(crate) fn toggle_record(&mut self) {
        if self.recording {
            self.end_take();
            self.status = "recording off".into();
            return;
        }
        self.recording = true;
        let mut status = format!("recording into track {}", self.cursor.track + 1);
        if !self.engine.is_playing() {
            if self.count_in_bars > 0 {
                self.send(RtCommand::Seek {
                    beats: self.marker_beats,
                });
                self.send(RtCommand::PlayAfterCountIn {
                    beats: (f32::from(self.count_in_bars) * BEATS_PER_BAR) as u16,
                });
                status = format!("counting in {} bar(s), then {status}", self.count_in_bars);
            } else {
                self.toggle_play();
            }
        }
        self.status = status;
    }

    /// Turn the metronome click on or off.
    pub(crate) fn toggle_metronome(&mut self) {
        self.metronome = !self.metronome;
        self.send(RtCommand::SetMetronome { on: self.metronome });
        self.status = format!("metronome {}", if self.metronome { "on" } else { "off" });
    }

    /// Set how many bars a recording from stop counts in: 0, 1, 2 or 4.
    pub(crate) fn set_count_in(&mut self, bars: u8) {
        if !COUNT_IN_BARS.contains(&bars) {
            self.status = "count-in must be 0, 1, 2 or 4 bars".into();
            return;
        }
        self.count_in_bars = bars;
        self.status = match bars {
            0 => "count-in off".into(),
            1 => "count-in 1 bar".into(),
            _ => format!("count-in {bars} bars"),
        };
    }

    /// Set the grid recorded notes snap to, or `0` to record them as played.
    pub(crate) fn set_record_quantize(&mut self, grid_beats: f32) {
        if grid_beats == 0.0 {
            self.record_quantize_beats = 0.0;
            self.status = "record quantize off".into();
            return;
        }
        if !(MIN_RECORD_GRID..=MAX_RECORD_GRID).contains(&grid_beats) {
            self.status = "record quantize must be off or 1/16 to 4 beats".into();
            return;
        }
        self.record_quantize_beats = grid_beats;
        self.status = format!("record quantize {}", grid_name(grid_beats));
    }

    /// Whether the metronome is still counting in before the transport starts.
    fn counting_in(&self) -> bool {
        self.engine.shared().count_in_frames.load(Ordering::Relaxed) > 0
    }

    /// Beat a note played now starts on in the take, or `None` when nothing is
    /// recording or the count-in has not reached the downbeat.
    fn take_start(&self, counting_in: bool) -> Option<f32> {
        (self.recording && !counting_in).then(|| self.position_beats())
    }

    /// Close the take: notes still held are written up to the playhead.
    fn end_take(&mut self) {
        if !self.recording {
            return;
        }
        let now = self.position_beats();
        for index in 0..self.held.len() {
            if let Some(start) = self.held[index].recorded_from.take() {
                let note = &self.held[index];
                let (track, pitch, velocity) = (note.track as usize, note.pitch, note.velocity);
                self.record_note(track, pitch, velocity, start, now);
            }
        }
        self.recording = false;
        self.take = None;
    }

    /// The transport position, in beats.
    fn position_beats(&self) -> f32 {
        (self.engine.position_frames() as f64 / self.frames_per_beat()) as f32
    }

    /// Write one played note into the take. It lands in the MIDI block under
    /// its start, else in the block the take is growing, else in a new block
    /// that starts on the note's bar. A note held across a loop wrap ends at the
    /// loop end.
    pub(crate) fn record_note(
        &mut self,
        track: usize,
        pitch: u8,
        velocity: u8,
        start: f32,
        end: f32,
    ) {
        let Some(kind) = self.song.tracks.get(track).map(|lane| lane.kind) else {
            return;
        };
        if kind != TrackKind::Midi {
            self.status = "recording needs a MIDI track".into();
            return;
        }
        let end = if end > start {
            end
        } else {
            self.song
                .loop_region()
                .map_or(start, |region| region.end_beats)
        };
        let (start, end) = snap_take(start, end, self.record_quantize_beats);
        let lane = &self.song.tracks[track];
        let covering = lane.blocks.iter().position(|block| {
            block.kind == TrackKind::Midi && block.start_beats <= start && start < block.end_beats()
        });
        let growing = self
            .take
            .filter(|take| take.track == track)
            .and_then(|take| {
                lane.blocks.iter().position(|block| {
                    block.kind == TrackKind::Midi
                        && block.start_beats == take.start_beats
                        && block.start_beats <= start
                })
            });
        let index = match covering.or(growing) {
            Some(index) => index,
            None => {
                if !self.has_room(track) {
                    return;
                }
                let start_beats = (start / BEATS_PER_BAR).floor() * BEATS_PER_BAR;
                self.take = Some(Take { track, start_beats });
                self.song.tracks[track].add_block(start_beats, BEATS_PER_BAR)
            }
        };
        let block = &mut self.song.tracks[track].blocks[index];
        if block.notes.len() >= MAX_NOTES_PER_BLOCK {
            self.status = format!("block is already at {MAX_NOTES_PER_BLOCK} notes");
            return;
        }
        let growing = Take {
            track,
            start_beats: block.start_beats,
        };
        if self.take == Some(growing) {
            let bars = ((end - block.start_beats) / BEATS_PER_BAR).ceil() * BEATS_PER_BAR;
            if bars > block.length_beats {
                block.set_length(bars);
            }
        }
        let mut note = Note {
            pitch,
            start_beats: start - block.start_beats,
            length_beats: (end - start).max(Note::MIN_LENGTH_BEATS),
            velocity,
        };
        note.clamp_to(block.length_beats);
        block.notes.push(note);
        self.cursor = Cursor {
            track,
            block: index,
        };
        self.push_mirror(track);
        self.push_notes(track);
        self.status = format!("recorded note {pitch}");
    }

    /// Enable or disable looping.
    pub(crate) fn toggle_loop(&mut self) {
        let looping = !self.engine.shared().looping.load(Ordering::Relaxed);
        self.send(RtCommand::SetLooping { looping });
        self.status = format!("looping {}", if looping { "on" } else { "off" });
    }

    /// Set the loop region, or clear it when the span is too short to loop.
    pub(crate) fn set_loop_region(&mut self, start_beats: f32, end_beats: f32) {
        let region = LoopRegion::between(start_beats, end_beats);
        self.song.set_loop_region(region);
        self.send(RtCommand::loop_region(region));
        self.status = match region {
            Some(region) => format!(
                "loop {:.2} to {:.2} beats",
                region.start_beats, region.end_beats
            ),
            None => "loop over the whole song".into(),
        };
    }

    /// Change the tempo.
    pub(crate) fn nudge_bpm(&mut self, delta: f32) {
        self.song.set_bpm(self.song.bpm() + delta);
        self.send(RtCommand::SetBpm {
            bpm: self.song.bpm(),
        });
        self.status = format!("bpm {:.0}", self.song.bpm());
    }

    /// Transpose the play keyboard.
    pub(crate) fn nudge_octave(&mut self, delta: i32) {
        self.octave = (self.octave + delta).clamp(-4, 4);
        self.status = format!("keyboard octave {:+}", self.octave);
    }

    /// Open the block-name prompt, or commit the one already open.
    pub(crate) fn rename_block(&mut self) {
        if self.rename.is_some() {
            self.rename_commit();
            return;
        }
        let Some(name) = self
            .song
            .tracks
            .get(self.cursor.track)
            .and_then(|track| track.blocks.get(self.cursor.block))
            .map(|block| block.name.clone())
        else {
            self.status = "move to a block to rename it".into();
            return;
        };
        self.rename = Some(String::new());
        self.status = format!("renaming {name}: type a new name, enter to confirm, esc to cancel");
    }

    /// Replace the block's name with what the prompt holds.
    pub(crate) fn rename_commit(&mut self) {
        let Some(buffer) = self.rename.take() else {
            return;
        };
        let name = buffer.trim().to_string();
        if name.is_empty() {
            self.status = "rename left unchanged".into();
            return;
        }
        if let Some(block) = self
            .song
            .tracks
            .get_mut(self.cursor.track)
            .and_then(|track| track.blocks.get_mut(self.cursor.block))
        {
            block.name = name.clone();
        }
        self.status = format!("block renamed to {name}");
    }

    /// Abandon the block-name prompt.
    pub(crate) fn cancel_rename(&mut self) {
        if self.rename.take().is_some() {
            self.status = "rename cancelled".into();
        }
    }

    /// A character typed into the block-name prompt.
    pub(crate) fn type_character(&mut self, character: char) {
        let Some(buffer) = self.rename.as_mut() else {
            return;
        };
        if buffer.chars().count() >= MAX_NAME_LEN {
            return;
        }
        buffer.push(character);
        self.status = format!("rename: {buffer}");
    }

    /// A backspace in the block-name prompt.
    pub(crate) fn backspace(&mut self) {
        if let Some(buffer) = self.rename.as_mut() {
            buffer.pop();
            self.status = format!("rename: {buffer}");
        }
    }

    /// A note was played on the keyboard.
    pub(crate) fn play_note(&mut self, pitch: u8, velocity: u8) {
        let track = self.cursor.track as u8;
        self.send(RtCommand::NoteOn {
            track,
            pitch,
            velocity,
        });
        let recorded_from = self.take_start(self.counting_in());
        self.held.push(HeldNote {
            track,
            pitch,
            velocity,
            recorded_from,
        });
    }

    /// Release every held note at `pitch`. The cursor may have moved since the
    /// key went down, so the note-off goes to the track that took the note-on.
    pub(crate) fn release_note(&mut self, pitch: u8) {
        let mut index = 0;
        while index < self.held.len() {
            if self.held[index].pitch == pitch {
                let note = self.held.swap_remove(index);
                self.send(RtCommand::NoteOff {
                    track: note.track,
                    pitch,
                });
                if let Some(start) = note.recorded_from {
                    let now = self.position_beats();
                    self.record_note(note.track as usize, pitch, note.velocity, start, now);
                }
            } else {
                index += 1;
            }
        }
    }

    /// Frames in one beat at the current tempo and the engine's rate.
    pub(crate) fn frames_per_beat(&self) -> f64 {
        Tempo::new(self.song.bpm()).frames_per_beat(self.engine.sample_rate())
    }
}

/// Snap a played note's start to the nearest line of `grid` beats, keeping its
/// length, and keep its end at least one grid step past the new start. A grid
/// of `0` leaves the note as played.
fn snap_take(start: f32, end: f32, grid: f32) -> (f32, f32) {
    if grid <= 0.0 {
        return (start, end);
    }
    let snapped = (start / grid).round() * grid;
    let moved = snapped + (end - start);
    (
        snapped,
        if moved > snapped {
            moved
        } else {
            snapped + grid
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    #[test]
    fn a_note_is_released_on_the_track_that_played_it() {
        let mut app = headless_app();
        app.handle(Event::NoteOn {
            pitch: 60,
            velocity: 100,
        });
        app.handle(Event::MoveTrack(1));
        app.handle(Event::NoteOff { pitch: 60 });
        assert!(
            app.held.is_empty(),
            "moving the cursor must not strand the held note"
        );
    }

    #[test]
    fn a_held_note_rings_until_its_release() {
        let mut app = headless_app();
        app.handle(Event::NoteOn {
            pitch: 64,
            velocity: 90,
        });
        app.tick();
        assert_eq!(app.held.len(), 1, "a held key must not time out");
    }

    #[test]
    fn a_take_opens_a_bar_block_and_grows_it() {
        let mut app = headless_app();
        app.recording = true;
        app.record_note(0, 60, 100, 5.0, 6.0);
        app.record_note(0, 62, 90, 9.5, 10.0);
        let blocks = &app.song.tracks[0].blocks;
        assert_eq!(blocks.len(), 1, "one take, one block");
        assert_eq!(blocks[0].start_beats, 4.0);
        assert_eq!(blocks[0].length_beats, 8.0);
        assert_eq!(blocks[0].notes[0].start_beats, 1.0);
        assert_eq!(blocks[0].notes[1].start_beats, 5.5);
        assert_eq!(blocks[0].notes[1].velocity, 90);
    }

    #[test]
    fn a_take_overdubs_into_the_block_under_it() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.recording = true;
        app.record_note(0, 60, 100, 1.0, 2.0);
        let blocks = &app.song.tracks[0].blocks;
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].length_beats, 4.0);
        assert_eq!(blocks[0].notes.len(), 1);
    }

    #[test]
    fn record_quantize_snaps_the_start_and_keeps_the_length() {
        let mut app = headless_app();
        app.handle(Event::SetRecordQuantize { grid_beats: 1.0 });
        assert_eq!(app.ui_state().record_quantize_beats, 1.0);
        assert_eq!(app.status, "record quantize 1/4");
        app.recording = true;
        app.record_note(0, 60, 100, 5.4, 6.9);
        app.record_note(0, 62, 100, 6.6, 6.6);
        let notes = &app.song.tracks[0].blocks[0].notes;
        assert_eq!(notes[0].start_beats, 1.0);
        assert!((notes[0].length_beats - 1.5).abs() < 1e-5);
        assert_eq!(notes[1].start_beats, 3.0, "6.6 snaps to 7, one beat in");
        assert_eq!(notes[1].length_beats, 1.0, "a zero-length note gets a step");
        assert_eq!(snap_take(2.2, 3.0, 0.0), (2.2, 3.0));
    }

    #[test]
    fn record_quantize_refuses_grids_out_of_range() {
        let mut app = headless_app();
        for grid_beats in [0.01, 8.0, f32::NAN, -1.0] {
            app.handle(Event::SetRecordQuantize { grid_beats });
            assert_eq!(app.record_quantize_beats, 0.0, "{grid_beats}");
            assert!(app.status.contains("must be"), "{}", app.status);
        }
        app.handle(Event::SetRecordQuantize { grid_beats: 0.25 });
        app.handle(Event::SetRecordQuantize { grid_beats: 0.0 });
        assert_eq!(app.record_quantize_beats, 0.0);
        assert_eq!(app.status, "record quantize off");
    }

    #[test]
    fn the_count_in_takes_only_the_menu_lengths() {
        let mut app = headless_app();
        assert_eq!(app.ui_state().count_in_bars, 1, "one bar by default");
        for bars in [0, 2, 4] {
            app.handle(Event::SetCountIn { bars });
            assert_eq!(app.count_in_bars, bars);
        }
        for bars in [3, 5, 255] {
            app.handle(Event::SetCountIn { bars });
            assert_eq!(app.count_in_bars, 4, "{bars} is refused");
            assert!(app.status.contains("must be"), "{}", app.status);
        }
    }

    #[test]
    fn arming_from_stop_counts_in_and_notes_wait_for_the_downbeat() {
        let mut app = headless_app();
        assert_eq!(app.take_start(false), None, "not recording yet");
        app.handle(Event::ToggleRecord);
        assert!(app.recording);
        assert!(
            app.status.starts_with("counting in 1 bar"),
            "{}",
            app.status
        );
        assert_eq!(app.take_start(true), None, "played before the downbeat");
        assert!(app.take_start(false).is_some());
    }

    #[test]
    fn the_metronome_toggles_and_reaches_the_snapshot() {
        let mut app = headless_app();
        assert!(!app.ui_state().metronome);
        app.handle(Event::ToggleMetronome);
        assert!(app.ui_state().metronome);
        assert_eq!(app.status, "metronome on");
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.handle(Event::Undo);
        assert!(app.metronome, "an undo keeps the preference");
        assert!(!app.history.can_undo(), "the toggle took no undo step");
    }

    #[test]
    fn a_take_refuses_an_audio_track() {
        let mut app = headless_app();
        app.recording = true;
        app.record_note(2, 60, 100, 0.0, 1.0);
        assert!(app.song.tracks[2].blocks.is_empty());
        assert!(app.status.contains("MIDI"), "{}", app.status);
    }

    #[test]
    fn stop_returns_to_the_marker_then_to_the_top() {
        let mut app = headless_app();
        app.handle(Event::Seek { beats: 8.0 });
        app.handle(Event::Stop);
        assert_eq!(
            app.marker_beats, 0.0,
            "a stop while stopped goes to the top"
        );
        app.handle(Event::Seek { beats: 8.0 });
        assert_eq!(app.marker_beats, 8.0);
    }

    #[test]
    fn renaming_a_block_replaces_its_name() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::RenameBlock);
        assert!(app.rename.is_some());
        for character in "Verse".chars() {
            app.handle(Event::TextInput(character));
        }
        app.handle(Event::RenameBlock);
        assert_eq!(app.song.tracks[0].blocks[0].name, "Verse");
        assert!(app.rename.is_none());
    }

    #[test]
    fn the_name_prompt_forgets_what_it_did_not_keep() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::RenameBlock);
        app.handle(Event::TextInput('X'));
        app.handle(Event::TextBackspace);
        assert_eq!(app.rename.as_deref(), Some(""));
        for _ in 0..MAX_NAME_LEN + 4 {
            app.handle(Event::TextInput('A'));
        }
        assert_eq!(app.rename.as_deref().map(str::len), Some(MAX_NAME_LEN));
    }

    #[test]
    fn cancelling_a_rename_keeps_the_old_name() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.handle(Event::RenameBlock);
        app.handle(Event::TextInput('X'));
        app.handle(Event::CancelRename);
        assert_eq!(app.song.tracks[0].blocks[0].name, "1");
    }

    #[test]
    fn the_octave_never_leaves_the_keyboard_range() {
        let mut app = headless_app();
        for _ in 0..9 {
            app.handle(Event::OctaveDelta(1));
        }
        assert_eq!(app.octave, 4);
        assert_eq!(app.ui_state().octave, 4);
    }

    #[test]
    fn the_tempo_moves_and_reaches_the_engine() {
        let mut app = headless_app();
        app.handle(Event::BpmDelta(8.0));
        assert_eq!(app.song.bpm(), 128.0);
        assert!(app.ui_state().status.contains("128"));
    }

    #[test]
    fn play_then_stop_reaches_the_engine() {
        let mut app = headless_app();
        app.handle(Event::TogglePlay);
        assert!(app.status.contains("playing"), "status: {}", app.status);
        app.handle(Event::Stop);
        assert!(app.status.contains("stopped"), "status: {}", app.status);
    }
}
