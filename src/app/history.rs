//! Undo and redo: whole-song snapshots taken before each edit.
//!
//! An edit is recorded only when it changed the song, so a refused edit leaves
//! no empty step. A drag or a fader arrives as a stream of events; the stream
//! of one gesture collapses into a single step, which any other event or an
//! explicit [`Event::Checkpoint`] closes. The loop region and the transport are
//! not part of the history: an undo keeps them where they are.

use super::App;
use crate::core::meter::MeterBank;
use crate::events::Event;
use crate::models::Song;
use crate::ui::view::Cursor;

/// Most steps kept in either direction.
pub(crate) const MAX_STEPS: usize = 200;

/// The song and the cursor as they stood before an edit.
#[derive(Clone, Debug)]
struct Step {
    song: Song,
    cursor: Cursor,
}

/// What makes consecutive events one gesture: the same kind of edit on the
/// same target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gesture {
    PlaceBlock,
    ResizeBlock,
    TrimBlockStart,
    StretchBlock,
    MoveNote(usize),
    ResizeNote(usize),
    NoteVelocity(usize),
    EditNotes(usize, usize),
    Mix(usize),
    Instrument(usize),
    InstrumentTone(usize),
    Volume(usize),
    Pan(usize),
    Bpm,
    Take,
    EffectParam(usize, usize, usize),
}

/// How an event takes part in the history.
enum Kind {
    /// Leaves the song alone, so it is never recorded.
    Untracked,
    /// One edit, one step.
    Edit,
    /// Part of a stream that collapses into one step.
    Stream(Gesture),
}

/// The undo and redo stacks, plus the gesture still collecting events.
#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    gesture: Option<Gesture>,
}

impl History {
    /// Whether an undo has a step to go back to.
    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether a redo has a step to go forward to.
    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn record(&mut self, step: Step, gesture: Option<Gesture>) {
        if self.undo.len() == MAX_STEPS {
            self.undo.remove(0);
        }
        self.undo.push(step);
        self.redo.clear();
        self.gesture = gesture;
    }

    fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.gesture = None;
    }
}

fn kind(event: &Event, cursor_track: usize, recording: bool) -> Kind {
    match event {
        Event::NoteOn { .. } | Event::NoteOff { .. } | Event::Stop | Event::ToggleRecord
            if recording =>
        {
            Kind::Stream(Gesture::Take)
        }
        Event::CreateBlock { .. }
        | Event::DeleteBlockAt { .. }
        | Event::SplitBlock { .. }
        | Event::DuplicateBlock { .. }
        | Event::AddNote { .. }
        | Event::DeleteNote { .. }
        | Event::AddNotes { .. }
        | Event::DeleteNotes { .. }
        | Event::AddBlock
        | Event::DeleteBlock
        | Event::ImportSample { .. }
        | Event::CycleTrackKind
        | Event::RenameBlock
        | Event::ToggleMute
        | Event::ToggleSolo
        | Event::SetPreset { .. }
        | Event::QuantizeBlock { .. }
        | Event::AddEffect { .. }
        | Event::RemoveEffect { .. }
        | Event::MoveEffect { .. }
        | Event::SetEffectEnabled { .. }
        | Event::SetEffectSidechain { .. }
        | Event::AddTrack { .. }
        | Event::DeleteTrack { .. }
        | Event::DuplicateTrack { .. }
        | Event::RenameTrack { .. }
        | Event::CutRange { .. }
        | Event::DeleteRange { .. }
        | Event::PasteRange { .. }
        | Event::DuplicateRange { .. }
        | Event::SetInstrumentEnabled { .. } => Kind::Edit,
        Event::SetEffectParam {
            track,
            index,
            param,
            ..
        } => Kind::Stream(Gesture::EffectParam(*track, *index, *param)),
        Event::PlaceBlock { .. } => Kind::Stream(Gesture::PlaceBlock),
        Event::ResizeBlock { .. } => Kind::Stream(Gesture::ResizeBlock),
        Event::TrimBlockStart { .. } => Kind::Stream(Gesture::TrimBlockStart),
        Event::StretchBlock { .. } => Kind::Stream(Gesture::StretchBlock),
        Event::MoveNote { note, .. } => Kind::Stream(Gesture::MoveNote(*note)),
        Event::ResizeNote { note, .. } => Kind::Stream(Gesture::ResizeNote(*note)),
        Event::SetNoteVelocity { note, .. } => Kind::Stream(Gesture::NoteVelocity(*note)),
        Event::EditNotes { track, block, .. } => Kind::Stream(Gesture::EditNotes(*track, *block)),
        Event::SetTrackMix { track, .. } => Kind::Stream(Gesture::Mix(*track)),
        Event::SetInstrument { track, .. } => Kind::Stream(Gesture::Instrument(*track)),
        Event::SetInstrumentTone { track, .. } => Kind::Stream(Gesture::InstrumentTone(*track)),
        Event::VolumeDelta(_) => Kind::Stream(Gesture::Volume(cursor_track)),
        Event::PanDelta(_) => Kind::Stream(Gesture::Pan(cursor_track)),
        Event::BpmDelta(_) => Kind::Stream(Gesture::Bpm),
        _ => Kind::Untracked,
    }
}

impl App {
    /// Apply one intent, recording it in the history when it edits the song.
    pub(crate) fn handle(&mut self, event: Event) {
        if matches!(event, Event::LoadProject { .. }) {
            let before = self.song.clone();
            self.apply(event);
            if self.song != before {
                self.history.clear();
            }
            return;
        }
        let gesture = match kind(&event, self.cursor.track, self.recording) {
            Kind::Untracked => {
                self.history.gesture = None;
                return self.apply(event);
            }
            Kind::Edit => None,
            Kind::Stream(gesture) => Some(gesture),
        };
        if gesture.is_some() && self.history.gesture == gesture {
            let before = self.song.clone();
            self.apply(event);
            if self.song != before {
                self.edit_revision += 1;
            }
            return;
        }
        let before = Step {
            song: self.song.clone(),
            cursor: self.cursor,
        };
        self.apply(event);
        if self.song != before.song {
            self.edit_revision += 1;
            self.history.record(before, gesture);
        } else {
            self.history.gesture = None;
        }
    }

    /// Step back to the song before the last edit.
    pub(crate) fn undo(&mut self) {
        self.history.gesture = None;
        let Some(step) = self.history.undo.pop() else {
            self.status = "nothing to undo".into();
            return;
        };
        let current = self.restore(step);
        self.history.redo.push(current);
        self.edit_revision += 1;
        self.status = format!("undo ({} more)", self.history.undo.len());
    }

    /// Step forward to the song an undo went back from.
    pub(crate) fn redo(&mut self) {
        self.history.gesture = None;
        let Some(step) = self.history.redo.pop() else {
            self.status = "nothing to redo".into();
            return;
        };
        let current = self.restore(step);
        self.history.undo.push(current);
        self.edit_revision += 1;
        self.status = format!("redo ({} more)", self.history.redo.len());
    }

    /// Put a step's song in place, keeping the loop region, and hand back the
    /// song it replaced.
    fn restore(&mut self, step: Step) -> Step {
        let mut song = step.song;
        song.set_loop_region(self.song.loop_region());
        let replaced = Step {
            song: std::mem::replace(&mut self.song, song),
            cursor: self.cursor,
        };
        self.rename = None;
        self.cursor = step.cursor;
        self.cursor.track = self
            .cursor
            .track
            .min(self.song.tracks.len().saturating_sub(1));
        self.clamp_block();
        if self.song.tracks.len() != replaced.song.tracks.len() {
            self.meters = MeterBank::new(self.song.tracks.len());
        }
        self.resync();
        replaced
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::events::Event;

    fn headless_app() -> App {
        App::new_silent().expect("silent engine")
    }

    fn block_count(app: &App) -> usize {
        app.song.tracks[0].blocks.len()
    }

    fn create(app: &mut App, start_beats: f32) {
        app.submit(Event::CreateBlock {
            track: 0,
            start_beats,
            length_beats: 4.0,
        });
    }

    #[test]
    fn undo_and_redo_walk_the_edits() {
        let mut app = headless_app();
        create(&mut app, 0.0);
        create(&mut app, 8.0);
        app.submit(Event::Undo);
        assert_eq!(block_count(&app), 1);
        app.submit(Event::Undo);
        assert_eq!(block_count(&app), 0);
        app.submit(Event::Redo);
        app.submit(Event::Redo);
        assert_eq!(block_count(&app), 2);
        assert!(!app.history.can_redo());
    }

    #[test]
    fn a_whole_take_is_one_undo_step() {
        let mut app = headless_app();
        app.submit(Event::SetCountIn { bars: 0 });
        app.submit(Event::ToggleRecord);
        for pitch in [60, 64] {
            app.submit(Event::NoteOn {
                pitch,
                velocity: 100,
            });
            app.submit(Event::NoteOff { pitch });
        }
        app.submit(Event::Stop);
        assert_eq!(app.song.tracks[0].blocks[0].notes.len(), 2);
        app.submit(Event::Undo);
        assert_eq!(block_count(&app), 0);
        assert!(!app.history.can_undo());
    }

    #[test]
    fn a_new_edit_drops_the_redo_steps() {
        let mut app = headless_app();
        create(&mut app, 0.0);
        app.submit(Event::Undo);
        create(&mut app, 4.0);
        assert!(!app.history.can_redo());
        assert_eq!(app.song.tracks[0].blocks[0].start_beats, 4.0);
    }

    #[test]
    fn a_refused_edit_records_nothing() {
        let mut app = headless_app();
        create(&mut app, 0.0);
        app.submit(Event::SplitBlock {
            track: 0,
            block: 0,
            at_beats: 40.0,
        });
        app.submit(Event::Undo);
        assert_eq!(
            block_count(&app),
            0,
            "the refused split must not use a step"
        );
    }

    #[test]
    fn a_drag_is_one_step_and_a_new_gesture_is_another() {
        let mut app = headless_app();
        create(&mut app, 0.0);
        for start_beats in [1.0, 2.0, 3.0] {
            app.submit(Event::PlaceBlock {
                track: 0,
                block: 0,
                target_track: 0,
                start_beats,
            });
        }
        app.submit(Event::Checkpoint);
        for start_beats in [5.0, 6.0] {
            app.submit(Event::PlaceBlock {
                track: 0,
                block: 0,
                target_track: 0,
                start_beats,
            });
        }
        app.submit(Event::Undo);
        assert_eq!(app.song.tracks[0].blocks[0].start_beats, 3.0);
        app.submit(Event::Undo);
        assert_eq!(app.song.tracks[0].blocks[0].start_beats, 0.0);
    }

    #[test]
    fn undo_keeps_the_loop_region() {
        let mut app = headless_app();
        create(&mut app, 0.0);
        app.submit(Event::SetLoopRegion {
            start_beats: 4.0,
            end_beats: 8.0,
        });
        app.submit(Event::Undo);
        assert_eq!(block_count(&app), 0);
        assert!(app.song.loop_region().is_some());
    }

    #[test]
    fn the_history_is_capped() {
        let mut app = headless_app();
        for step in 0..super::MAX_STEPS + 5 {
            let delta = if step % 2 == 0 { 1.0 } else { -1.0 };
            app.submit(Event::BpmDelta(delta));
            app.submit(Event::Checkpoint);
        }
        assert_eq!(app.history.undo.len(), super::MAX_STEPS);
    }
}
