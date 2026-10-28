//! Notes inside MIDI blocks: drawing, moving, resizing, velocity and deletion.
//!
//! Every edit lands on the block first, then the track's note mirror is rebuilt
//! and pushed, the same way a block edit pushes the block mirror. Notes stay in
//! the order they were drawn, so the index a drag holds keeps naming its note.

use super::{App, BEATS_PER_BAR};
use crate::models::{Block, MAX_NOTES_PER_BLOCK, Note, TrackKind};
use crate::ui::view::Cursor;

impl App {
    /// Draw a note into a MIDI block, clamped to fit inside it.
    pub(crate) fn add_note(
        &mut self,
        track: usize,
        block: usize,
        pitch: u8,
        start_beats: f32,
        length_beats: f32,
        velocity: u8,
    ) {
        let Some(target) = self.midi_block(track, block) else {
            return;
        };
        if target.notes.len() >= MAX_NOTES_PER_BLOCK {
            self.status = format!("block is already at {MAX_NOTES_PER_BLOCK} notes");
            return;
        }
        let mut note = Note {
            pitch,
            start_beats,
            length_beats,
            velocity,
        };
        note.clamp_to(target.length_beats);
        target.notes.push(note);
        self.finish_note_edit(track, block, format!("note {} added", note.pitch));
    }

    /// Move a note to a new pitch and start, keeping its length where it fits.
    pub(crate) fn move_note(
        &mut self,
        track: usize,
        block: usize,
        note: usize,
        pitch: u8,
        start_beats: f32,
    ) {
        let Some((limit, target)) = self.note_mut(track, block, note) else {
            return;
        };
        target.pitch = pitch;
        target.start_beats = start_beats;
        target.clamp_to(limit);
        let (pitch, start) = (target.pitch, target.start_beats);
        self.finish_note_edit(track, block, format!("note {pitch} at {start:.2}"));
    }

    /// Drag a note's right edge.
    pub(crate) fn resize_note(&mut self, track: usize, block: usize, note: usize, length: f32) {
        let Some((limit, target)) = self.note_mut(track, block, note) else {
            return;
        };
        target.length_beats = length;
        target.clamp_to(limit);
        let length = target.length_beats;
        self.finish_note_edit(track, block, format!("note length {length:.2} beats"));
    }

    /// Change how hard a note is struck.
    pub(crate) fn set_note_velocity(
        &mut self,
        track: usize,
        block: usize,
        note: usize,
        velocity: u8,
    ) {
        let Some((limit, target)) = self.note_mut(track, block, note) else {
            return;
        };
        target.velocity = velocity;
        target.clamp_to(limit);
        let velocity = target.velocity;
        self.finish_note_edit(track, block, format!("note velocity {velocity}"));
    }

    /// Remove one note. Later notes shift down one index.
    pub(crate) fn delete_note(&mut self, track: usize, block: usize, note: usize) {
        if self.note_mut(track, block, note).is_none() {
            return;
        }
        let removed = self.song.tracks[track].blocks[block].notes.remove(note);
        self.finish_note_edit(track, block, format!("note {} deleted", removed.pitch));
    }

    /// Snap every note start in a MIDI block to the timeline grid of
    /// `grid_beats`, keeping each length where the block has room for it.
    pub(crate) fn add_notes(&mut self, track: usize, block: usize, notes: &[Note]) {
        let Some(target) = self.midi_block(track, block) else {
            return;
        };
        if target.notes.len() + notes.len() > MAX_NOTES_PER_BLOCK {
            self.status = format!(
                "adding {} notes would exceed limit of {MAX_NOTES_PER_BLOCK}",
                notes.len()
            );
            return;
        }
        let limit = target.length_beats;
        for note in notes {
            let mut note = *note;
            note.clamp_to(limit);
            target.notes.push(note);
        }
        let count = notes.len();
        self.finish_note_edit(track, block, format!("{count} notes added"));
    }

    pub(crate) fn edit_notes(
        &mut self,
        track: usize,
        block: usize,
        edits: &[crate::events::NoteEdit],
    ) {
        let Some(target) = self.midi_block(track, block) else {
            return;
        };
        let limit = target.length_beats;
        let mut count = 0;
        for edit in edits {
            if let Some(note) = target.notes.get_mut(edit.note) {
                note.pitch = edit.pitch;
                note.start_beats = edit.start_beats;
                note.length_beats = edit.length_beats;
                note.velocity = edit.velocity;
                note.clamp_to(limit);
                count += 1;
            }
        }
        self.finish_note_edit(track, block, format!("{count} notes edited"));
    }

    pub(crate) fn delete_notes(&mut self, track: usize, block: usize, notes: &[usize]) {
        let Some(target) = self.midi_block(track, block) else {
            return;
        };
        let mut indices: Vec<usize> = notes.to_vec();
        indices.sort_unstable();
        indices.dedup();
        let mut count = 0;
        for &idx in indices.iter().rev() {
            if idx < target.notes.len() {
                target.notes.remove(idx);
                count += 1;
            }
        }
        self.finish_note_edit(track, block, format!("{count} notes deleted"));
    }

    pub(crate) fn quantize_block(
        &mut self,
        track: usize,
        block: usize,
        grid_beats: f32,
        notes: &[usize],
    ) {
        if !(grid_beats.is_finite() && grid_beats >= Note::MIN_LENGTH_BEATS) {
            self.status = format!(
                "quantize needs a grid of at least {}",
                grid_name(Note::MIN_LENGTH_BEATS)
            );
            return;
        }
        let Some(target) = self.midi_block(track, block) else {
            return;
        };
        let (offset, limit) = (target.start_beats, target.length_beats);
        let quantize_one = |note: &mut Note| {
            let snapped_start = (((offset + note.start_beats) / grid_beats).round() * grid_beats
                - offset)
                .clamp(0.0, (limit - Note::MIN_LENGTH_BEATS).max(0.0));
            let snapped_len = ((note.length_beats / grid_beats).round().max(1.0) * grid_beats)
                .clamp(
                    Note::MIN_LENGTH_BEATS,
                    (limit - snapped_start).max(Note::MIN_LENGTH_BEATS),
                );
            note.start_beats = snapped_start;
            note.length_beats = snapped_len;
            note.clamp_to(limit);
        };
        let count = if notes.is_empty() {
            for note in &mut target.notes {
                quantize_one(note);
            }
            target.notes.len()
        } else {
            let mut indices: Vec<usize> = notes.to_vec();
            indices.sort_unstable();
            indices.dedup();
            let mut modified = 0;
            for idx in indices {
                if let Some(note) = target.notes.get_mut(idx) {
                    quantize_one(note);
                    modified += 1;
                }
            }
            modified
        };
        let mut order: Vec<usize> = (0..target.notes.len()).collect();
        order.sort_by(|&a, &b| {
            target.notes[a]
                .pitch
                .cmp(&target.notes[b].pitch)
                .then_with(|| {
                    target.notes[a]
                        .start_beats
                        .total_cmp(&target.notes[b].start_beats)
                })
        });
        for pair in order.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if target.notes[a].pitch == target.notes[b].pitch {
                let b_start = target.notes[b].start_beats;
                let a_start = target.notes[a].start_beats;
                if b_start > a_start && a_start + target.notes[a].length_beats > b_start {
                    target.notes[a].length_beats = (b_start - a_start).max(Note::MIN_LENGTH_BEATS);
                    target.notes[a].clamp_to(limit);
                }
            }
        }
        let status = format!("quantized {count} notes to {}", grid_name(grid_beats));
        self.finish_note_edit(track, block, status);
    }

    /// The MIDI block a note edit names, or `None` with the reason in the status.
    fn midi_block(&mut self, track: usize, block: usize) -> Option<&mut Block> {
        let kind = self
            .song
            .tracks
            .get(track)
            .and_then(|lane| lane.blocks.get(block))
            .map(|target| target.kind);
        match kind {
            None => {
                self.status = "no block there to edit notes in".into();
                None
            }
            Some(TrackKind::Audio) => {
                self.status = "only MIDI blocks hold notes".into();
                None
            }
            Some(TrackKind::Midi) => Some(&mut self.song.tracks[track].blocks[block]),
        }
    }

    /// The note an edit names and the length of its block, or `None` with the
    /// reason in the status.
    fn note_mut(&mut self, track: usize, block: usize, note: usize) -> Option<(f32, &mut Note)> {
        let count = self.midi_block(track, block)?.notes.len();
        if note >= count {
            self.status = "no note there".into();
            return None;
        }
        let target = &mut self.song.tracks[track].blocks[block];
        Some((target.length_beats, &mut target.notes[note]))
    }

    /// Select the edited block, push its track's notes and report the edit.
    fn finish_note_edit(&mut self, track: usize, block: usize, status: String) {
        self.cursor = Cursor { track, block };
        self.push_notes(track);
        self.status = status;
    }
}

/// A grid in beats as a note fraction, `1.0` reading `1/4`, or in beats when
/// it is no whole fraction of a bar.
pub(super) fn grid_name(grid_beats: f32) -> String {
    let denominator = BEATS_PER_BAR / grid_beats;
    if denominator >= 1.0 && denominator.fract() == 0.0 {
        format!("1/{denominator}")
    } else {
        format!("{grid_beats} beats")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    fn app_with_block() -> App {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 8.0,
            length_beats: 4.0,
        });
        app
    }

    fn add(app: &mut App, pitch: u8, start_beats: f32, length_beats: f32, velocity: u8) {
        app.handle(Event::AddNote {
            track: 0,
            block: 0,
            pitch,
            start_beats,
            length_beats,
            velocity,
        });
    }

    fn notes(app: &App) -> &[Note] {
        &app.song.tracks[0].blocks[0].notes
    }

    #[test]
    fn adding_a_note_clamps_it_into_the_block_and_selects_the_block() {
        let mut app = app_with_block();
        app.cursor = Cursor { track: 1, block: 0 };
        add(&mut app, 200, 3.5, 9.0, 0);
        assert_eq!(
            notes(&app),
            &[Note {
                pitch: 127,
                start_beats: 3.5,
                length_beats: 0.5,
                velocity: 1,
            }]
        );
        assert_eq!(app.cursor, Cursor { track: 0, block: 0 });
        assert!(!app.resync_pending);
        assert_eq!(app.mirror_notes(0).len(), 1);
    }

    #[test]
    fn adding_refuses_audio_blocks_missing_blocks_and_full_blocks() {
        let mut app = app_with_block();
        app.handle(Event::CreateBlock {
            track: 2,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.handle(Event::AddNote {
            track: 2,
            block: 0,
            pitch: 60,
            start_beats: 0.0,
            length_beats: 1.0,
            velocity: 100,
        });
        assert!(app.song.tracks[2].blocks[0].notes.is_empty());
        assert!(app.status.contains("MIDI"), "status: {}", app.status);

        app.handle(Event::AddNote {
            track: 9,
            block: 0,
            pitch: 60,
            start_beats: 0.0,
            length_beats: 1.0,
            velocity: 100,
        });
        assert!(app.status.contains("no block"), "status: {}", app.status);

        for _ in 0..MAX_NOTES_PER_BLOCK + 1 {
            add(&mut app, 60, 0.0, 1.0, 100);
        }
        assert_eq!(notes(&app).len(), MAX_NOTES_PER_BLOCK);
        assert!(app.status.contains("512"), "status: {}", app.status);
    }

    #[test]
    fn moving_a_note_keeps_its_index_and_clamps_it() {
        let mut app = app_with_block();
        add(&mut app, 60, 0.0, 1.0, 100);
        add(&mut app, 62, 1.0, 2.0, 100);
        app.handle(Event::MoveNote {
            track: 0,
            block: 0,
            note: 1,
            pitch: 130,
            start_beats: 3.0,
        });
        assert_eq!(notes(&app)[0].pitch, 60, "the other note stays put");
        assert_eq!(notes(&app)[1].pitch, 127);
        assert_eq!(notes(&app)[1].start_beats, 3.0);
        assert_eq!(
            notes(&app)[1].length_beats,
            1.0,
            "it shrinks to the block end"
        );

        app.handle(Event::MoveNote {
            track: 0,
            block: 0,
            note: 2,
            pitch: 60,
            start_beats: 0.0,
        });
        assert!(app.status.contains("no note"), "status: {}", app.status);
    }

    #[test]
    fn resizing_a_note_stays_between_the_minimum_and_the_block_end() {
        let mut app = app_with_block();
        add(&mut app, 60, 1.0, 1.0, 100);
        let resize = |length_beats| Event::ResizeNote {
            track: 0,
            block: 0,
            note: 0,
            length_beats,
        };
        app.handle(resize(0.0));
        assert_eq!(notes(&app)[0].length_beats, Note::MIN_LENGTH_BEATS);
        app.handle(resize(99.0));
        assert_eq!(notes(&app)[0].length_beats, 3.0);
        app.handle(resize(f32::NAN));
        assert_eq!(notes(&app)[0].length_beats, Note::DEFAULT_LENGTH_BEATS);
    }

    #[test]
    fn velocity_stays_audible_and_in_midi_range() {
        let mut app = app_with_block();
        add(&mut app, 60, 0.0, 1.0, 100);
        let velocity = |velocity| Event::SetNoteVelocity {
            track: 0,
            block: 0,
            note: 0,
            velocity,
        };
        app.handle(velocity(0));
        assert_eq!(notes(&app)[0].velocity, 1);
        app.handle(velocity(255));
        assert_eq!(notes(&app)[0].velocity, 127);
        app.handle(velocity(64));
        assert_eq!(notes(&app)[0].velocity, 64);
    }

    fn quantize(app: &mut App, track: usize, grid_beats: f32) {
        app.handle(Event::QuantizeBlock {
            track,
            block: 0,
            grid_beats,
            notes: Vec::new(),
        });
    }

    #[test]
    fn quantizing_snaps_starts_and_lengths_and_trims_overlaps() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.5,
            length_beats: 4.0,
        });
        add(&mut app, 60, 0.2, 1.8, 100);
        add(&mut app, 60, 1.4, 0.9, 100);
        add(&mut app, 64, 3.6, 0.4, 100);
        quantize(&mut app, 0, 1.0);
        let starts: Vec<f32> = notes(&app).iter().map(|n| n.start_beats).collect();
        assert_eq!(starts, vec![0.5, 1.5, 3.5], "absolute 1, 2 and 4");
        let lengths: Vec<f32> = notes(&app).iter().map(|n| n.length_beats).collect();
        assert_eq!(
            lengths,
            vec![1.0, 1.0, 0.5],
            "lengths snap to grid and overlapping same-pitch notes trim"
        );
        assert_eq!(app.status, "quantized 3 notes to 1/4");
        app.handle(Event::Undo);
        assert_eq!(notes(&app)[0].start_beats, 0.2, "one undo step");
    }

    #[test]
    fn quantizing_refuses_audio_blocks_and_bad_grids() {
        let mut app = app_with_block();
        add(&mut app, 60, 0.3, 1.0, 100);
        for grid_beats in [0.0, 0.01, f32::NAN, f32::INFINITY] {
            quantize(&mut app, 0, grid_beats);
            assert!(app.status.contains("at least 1/64"), "{}", app.status);
        }
        assert_eq!(notes(&app)[0].start_beats, 0.3);
        app.handle(Event::CreateBlock {
            track: 2,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        quantize(&mut app, 2, 1.0);
        assert!(app.status.contains("MIDI"), "{}", app.status);
        quantize(&mut app, 0, 0.5);
        assert_eq!(app.status, "quantized 1 notes to 1/8");
        assert_eq!(grid_name(3.0), "3 beats");
    }

    #[test]
    fn deleting_a_note_shifts_the_later_ones_down() {
        let mut app = app_with_block();
        for pitch in [60, 62, 64] {
            add(&mut app, pitch, 0.0, 1.0, 100);
        }
        app.handle(Event::DeleteNote {
            track: 0,
            block: 0,
            note: 0,
        });
        let pitches: Vec<u8> = notes(&app).iter().map(|note| note.pitch).collect();
        assert_eq!(pitches, vec![62, 64]);
        assert_eq!(app.mirror_notes(0).len(), 2);

        app.handle(Event::DeleteNote {
            track: 0,
            block: 5,
            note: 0,
        });
        assert_eq!(notes(&app).len(), 2);
        assert!(app.status.contains("no block"), "status: {}", app.status);
    }

    #[test]
    fn batch_notes_add_edit_and_delete() {
        use crate::events::NoteEdit;
        let mut app = app_with_block();
        let new_notes = vec![
            Note {
                pitch: 60,
                start_beats: 0.0,
                length_beats: 1.0,
                velocity: 100,
            },
            Note {
                pitch: 64,
                start_beats: 1.0,
                length_beats: 5.0,
                velocity: 100,
            },
        ];
        app.handle(Event::AddNotes {
            track: 0,
            block: 0,
            notes: new_notes,
        });
        assert_eq!(notes(&app).len(), 2);
        assert_eq!(notes(&app)[1].length_beats, 3.0);
        assert_eq!(app.status, "2 notes added");

        app.handle(Event::EditNotes {
            track: 0,
            block: 0,
            edits: vec![
                NoteEdit {
                    note: 0,
                    pitch: 62,
                    start_beats: 0.5,
                    length_beats: 1.0,
                    velocity: 90,
                },
                NoteEdit {
                    note: 99,
                    pitch: 70,
                    start_beats: 0.0,
                    length_beats: 1.0,
                    velocity: 100,
                },
            ],
        });
        assert_eq!(notes(&app)[0].pitch, 62);
        assert_eq!(notes(&app)[0].start_beats, 0.5);
        assert_eq!(app.status, "1 notes edited");

        app.handle(Event::EditNotes {
            track: 0,
            block: 0,
            edits: vec![NoteEdit {
                note: 0,
                pitch: 63,
                start_beats: 0.5,
                length_beats: 1.0,
                velocity: 90,
            }],
        });
        assert_eq!(notes(&app)[0].pitch, 63);

        app.handle(Event::Undo);
        assert_eq!(notes(&app)[0].pitch, 60);

        app.handle(Event::DeleteNotes {
            track: 0,
            block: 0,
            notes: vec![1, 0, 1, 99],
        });
        assert!(notes(&app).is_empty());
        assert_eq!(app.status, "2 notes deleted");
    }

    #[test]
    fn quantizing_a_subset_only_moves_named_notes() {
        let mut app = app_with_block();
        add(&mut app, 60, 0.2, 1.0, 100);
        add(&mut app, 64, 1.2, 1.0, 100);
        add(&mut app, 67, 2.2, 1.0, 100);

        app.handle(Event::QuantizeBlock {
            track: 0,
            block: 0,
            grid_beats: 1.0,
            notes: vec![1],
        });
        assert_eq!(notes(&app)[0].start_beats, 0.2);
        assert_eq!(notes(&app)[1].start_beats, 1.0);
        assert_eq!(notes(&app)[2].start_beats, 2.2);
        assert_eq!(app.status, "quantized 1 notes to 1/4");
    }
}
