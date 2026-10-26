//! Blocks on the timeline: creation, placement, resizing, deletion, and the
//! mirror the audio thread reads.
//!
//! The song owns the truth. The audio side gets a flattened copy of one lane
//! whenever that lane changes, so a drag costs one small `Vec` and the render
//! path never looks at a [`Track`](crate::models::Track).

use super::App;
use crate::core::render::{MirroredBlock, MirroredNote};
use crate::core::sample::NO_SLOT;
use crate::core::synth::sample_gain;
use crate::core::{MAX_BLOCKS, MAX_NOTES};
use crate::models::{Block, TrackKind};
use crate::ui::view::Cursor;

impl App {
    /// Create a block at a position the UI asked for and select it.
    pub(crate) fn create_block(&mut self, track: usize, start_beats: f32, length_beats: f32) {
        let Some(track_index) = self.clamp_track(track) else {
            return;
        };
        if !self.has_room(track_index) {
            return;
        }
        let index = self.song.tracks[track_index].add_block(start_beats, length_beats);
        let name = self.song.tracks[track_index].blocks[index].name.clone();
        self.cursor = Cursor {
            track: track_index,
            block: index,
        };
        self.push_mirror(track_index);
        self.status = format!("block {name} added to track {}", track_index + 1);
    }

    /// Append a block past the last one on the selected track.
    pub(crate) fn add_block(&mut self) {
        let track_index = self.cursor.track;
        let Some(start) = self.song.tracks.get(track_index).map(|t| t.free_start()) else {
            return;
        };
        self.create_block(track_index, start, Block::DEFAULT_LENGTH_BEATS);
    }

    /// Move a block, possibly onto another track. This is the drag, so it stays
    /// O(1) plus the insertion sort of the lane it lands on.
    pub(crate) fn place_block(
        &mut self,
        track: usize,
        block: usize,
        target_track: usize,
        start_beats: f32,
    ) {
        let (Some(source), Some(target)) =
            (self.clamp_track(track), self.clamp_track(target_track))
        else {
            return;
        };
        if block >= self.song.tracks[source].blocks.len() {
            return;
        }
        let mut moved = self.song.tracks[source].blocks.remove(block);
        moved.set_start(start_beats);
        let name = moved.name.clone();
        let index = self.song.tracks[target].insert_block(moved);
        self.cursor = Cursor {
            track: target,
            block: index,
        };
        self.push_mirror(source);
        if target != source {
            self.push_mirror(target);
        }
        self.status = format!("{name} on track {}", target + 1);
    }

    /// Drag a block's right edge.
    pub(crate) fn resize_block(&mut self, track: usize, block: usize, length_beats: f32) {
        let Some(track_index) = self.clamp_track(track) else {
            return;
        };
        if block >= self.song.tracks[track_index].blocks.len() {
            self.status = "no block there to resize".into();
            return;
        }
        self.song.tracks[track_index].blocks[block].set_length(length_beats);
        let length = self.song.tracks[track_index].blocks[block].length_beats;
        self.push_mirror(track_index);
        self.status = format!("block length {length:.2} beats");
    }

    /// Cut a block in two at an absolute beat and put the cursor on the right
    /// half, which keeps playing the material from where the cut fell.
    pub(crate) fn split_block(&mut self, track: usize, block: usize, at_beats: f32) {
        let Some(track_index) = self.editable_block(track, block, "split") else {
            return;
        };
        if !self.has_room(track_index) {
            return;
        }
        let seconds_per_beat = self.seconds_per_beat();
        let lane = &mut self.song.tracks[track_index];
        let name = lane.next_name();
        let Some(right) = lane.blocks[block].split_off(at_beats, seconds_per_beat, name) else {
            self.status = "split inside the block, away from its edges".into();
            return;
        };
        let right_name = right.name.clone();
        let index = lane.insert_block(right);
        self.cursor = Cursor {
            track: track_index,
            block: index,
        };
        self.push_mirror(track_index);
        self.status = format!("split off block {right_name}");
    }

    /// Copy a block onto the beat right after it and put the cursor on the copy.
    pub(crate) fn duplicate_block(&mut self, track: usize, block: usize) {
        let Some(track_index) = self.editable_block(track, block, "duplicate") else {
            return;
        };
        if !self.has_room(track_index) {
            return;
        }
        let lane = &mut self.song.tracks[track_index];
        let mut copy = lane.blocks[block].clone();
        copy.name = lane.next_name();
        copy.set_start(copy.end_beats());
        let name = copy.name.clone();
        let index = lane.insert_block(copy);
        self.cursor = Cursor {
            track: track_index,
            block: index,
        };
        self.push_mirror(track_index);
        self.status = format!("block {name} duplicated");
    }

    /// Drag a block's left edge: its end and its material stay where they are.
    pub(crate) fn trim_block_start(&mut self, track: usize, block: usize, start_beats: f32) {
        let Some(track_index) = self.editable_block(track, block, "trim") else {
            return;
        };
        let seconds_per_beat = self.seconds_per_beat();
        let lane = &mut self.song.tracks[track_index];
        let mut trimmed = lane.blocks.remove(block);
        trimmed.trim_start(start_beats, seconds_per_beat);
        let start = trimmed.start_beats;
        let index = lane.insert_block(trimmed);
        self.cursor = Cursor {
            track: track_index,
            block: index,
        };
        self.push_mirror(track_index);
        self.status = format!("block starts at beat {start:.2}");
    }

    /// Drag a block's right edge so its material fills the new length.
    pub(crate) fn stretch_block(&mut self, track: usize, block: usize, length_beats: f32) {
        let Some(track_index) = self.editable_block(track, block, "stretch") else {
            return;
        };
        let stretched = &mut self.song.tracks[track_index].blocks[block];
        stretched.stretch_to(length_beats);
        let (length, rate) = (stretched.length_beats, stretched.rate);
        self.push_mirror(track_index);
        self.status = format!("block length {length:.2} beats at {rate:.2}x");
    }

    /// Remove one block from one track, wherever the cursor is.
    pub(crate) fn delete_block_at(&mut self, track: usize, block: usize) {
        let Some(track_index) = self.clamp_track(track) else {
            return;
        };
        if block >= self.song.tracks[track_index].blocks.len() {
            return;
        }
        let removed = self.song.tracks[track_index].blocks.remove(block);
        if self.cursor.track == track_index {
            self.clamp_block();
        }
        self.push_mirror(track_index);
        self.status = format!("block {} deleted", removed.name);
    }

    /// Remove the block under the cursor.
    pub(crate) fn delete_block(&mut self) {
        let track_index = self.cursor.track;
        let block = self.cursor.block;
        let Some(track) = self.song.tracks.get(track_index) else {
            return;
        };
        if block >= track.blocks.len() {
            self.status = "move onto a block to delete it".into();
            return;
        }
        let removed = self.song.tracks[track_index].blocks.remove(block);
        self.clamp_block();
        self.push_mirror(track_index);
        self.status = format!("block {} deleted", removed.name);
    }

    /// Flatten one lane for the audio thread: span, sample slot and the gain the
    /// voice starts at.
    pub(crate) fn mirror(&self, track: usize) -> Vec<MirroredBlock> {
        let Some(source) = self.song.tracks.get(track) else {
            return Vec::new();
        };
        source
            .blocks
            .iter()
            .map(|block| {
                if block.kind == TrackKind::Midi {
                    return MirroredBlock {
                        start_beats: block.start_beats,
                        end_beats: block.end_beats(),
                        slot: NO_SLOT,
                        gain: 0.0,
                        offset_seconds: 0.0,
                        rate: 1.0,
                    };
                }
                MirroredBlock {
                    start_beats: block.start_beats,
                    end_beats: block.end_beats(),
                    slot: self.samples.get(&block.source).copied().unwrap_or(NO_SLOT),
                    gain: sample_gain(&source.instrument),
                    offset_seconds: block.offset_seconds,
                    rate: block.rate,
                }
            })
            .collect()
    }

    /// Flatten one lane's notes for the audio thread: absolute beats, MIDI
    /// blocks only, each note cut at its block's end and a note that starts at
    /// or past the end dropped, sorted by start and capped at [`MAX_NOTES`].
    pub(crate) fn mirror_notes(&self, track: usize) -> Vec<MirroredNote> {
        let Some(source) = self.song.tracks.get(track) else {
            return Vec::new();
        };
        let mut notes: Vec<MirroredNote> = source
            .blocks
            .iter()
            .filter(|block| block.kind == TrackKind::Midi)
            .flat_map(|block| {
                block
                    .notes
                    .iter()
                    .filter(|note| note.start_beats < block.length_beats)
                    .map(|note| {
                        let start_beats = block.start_beats + note.start_beats;
                        MirroredNote {
                            start_beats,
                            end_beats: (start_beats + note.length_beats).min(block.end_beats()),
                            pitch: note.pitch,
                            velocity: note.velocity,
                        }
                    })
            })
            .collect();
        notes.sort_by(|a, b| a.start_beats.total_cmp(&b.start_beats));
        notes.truncate(MAX_NOTES);
        notes
    }

    /// Replace one lane's mirror on the audio side, blocks and notes. Call it
    /// after any edit that changes a lane: a moved block moves its notes too.
    pub(crate) fn push_mirror(&mut self, track: usize) {
        let blocks = self.mirror(track);
        if !self.engine.set_blocks(track, blocks) {
            self.resync_pending = true;
        }
        self.push_notes(track);
    }

    /// Replace one lane's note mirror on the audio side. Call it after a note
    /// edit, which leaves the blocks as they were.
    pub(crate) fn push_notes(&mut self, track: usize) {
        self.touch_notes(track);
        let notes = self.mirror_notes(track);
        if !self.engine.set_notes(track, notes) {
            self.resync_pending = true;
        }
    }

    /// Push every lane's mirror, after a resync has replaced them with empty
    /// ones. `AudioEngine::resync` can only see the song, so the sample slots,
    /// the voice gains and the notes it cannot know have to come back from here.
    pub(crate) fn repush_lanes(&mut self) {
        for track in 0..self.song.tracks.len() {
            self.touch_notes(track);
        }
        for track in 0..self.song.tracks.len() {
            let blocks = self.mirror(track);
            let notes = self.mirror_notes(track);
            if !self.engine.set_blocks(track, blocks) || !self.engine.set_notes(track, notes) {
                self.resync_pending = true;
                return;
            }
        }
    }

    /// Give one track's notes a fresh revision, after anything that may have
    /// changed them or the blocks that hold them.
    fn touch_notes(&mut self, track: usize) {
        if let Some(revision) = self.note_revisions.get_mut(track) {
            self.note_revision += 1;
            *revision = self.note_revision;
        }
    }

    /// The track of a block the UI named, or `None` with a status saying there
    /// is no block there to `verb`.
    fn editable_block(&mut self, track: usize, block: usize, verb: &str) -> Option<usize> {
        let track_index = self.clamp_track(track)?;
        if block >= self.song.tracks[track_index].blocks.len() {
            self.status = format!("no block there to {verb}");
            return None;
        }
        Some(track_index)
    }

    /// Whether a track can take one more block, setting the status when not.
    fn has_room(&mut self, track: usize) -> bool {
        if self.song.tracks[track].blocks.len() < MAX_BLOCKS {
            return true;
        }
        self.status = format!("track {} is already at {MAX_BLOCKS} blocks", track + 1);
        false
    }

    /// Seconds one beat lasts at the song's tempo.
    fn seconds_per_beat(&self) -> f32 {
        60.0 / self.song.bpm().max(1.0)
    }

    /// Clamp a track index from the UI to a real track.
    fn clamp_track(&self, track: usize) -> Option<usize> {
        if self.song.tracks.is_empty() {
            return None;
        }
        Some(track.min(self.song.tracks.len() - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;
    use crate::models::{MAX_LENGTH_BEATS, Note};
    use crate::ui::view::Cursor;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    #[test]
    fn adding_a_block_appends_it_and_moves_the_cursor() {
        let mut app = headless_app();
        app.cursor = Cursor { track: 0, block: 0 };
        app.handle(Event::AddBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(app.cursor.block, 0);
        assert_eq!(app.song.tracks[0].blocks[0].length_beats, 4.0);

        app.cursor.block = 1;
        app.handle(Event::AddBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 2);
        assert_eq!(app.cursor.block, 1);
    }

    #[test]
    fn a_created_block_lands_where_the_ui_asked() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 16.0,
            length_beats: 8.0,
        });
        let block = &app.song.tracks[0].blocks[0];
        assert_eq!(block.start_beats, 16.0);
        assert_eq!(block.length_beats, 8.0);
        assert_eq!(app.cursor, Cursor { track: 0, block: 0 });
    }

    #[test]
    fn a_created_block_is_clamped_to_the_song() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: -8.0,
            length_beats: 0.0,
        });
        let block = &app.song.tracks[0].blocks[0];
        assert_eq!(block.start_beats, 0.0);
        assert_eq!(block.length_beats, Block::MIN_LENGTH_BEATS);
    }

    #[test]
    fn a_created_block_takes_the_kind_of_its_track() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 2,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        assert_eq!(app.song.tracks[2].blocks[0].kind, TrackKind::Audio);
        assert_eq!(app.song.tracks[0].blocks.len(), 0);
    }

    #[test]
    fn a_track_refuses_to_grow_past_its_block_ceiling() {
        let mut app = headless_app();
        for _ in 0..MAX_BLOCKS {
            app.handle(Event::AddBlock);
        }
        assert_eq!(app.song.tracks[0].blocks.len(), MAX_BLOCKS);

        app.handle(Event::AddBlock);
        assert_eq!(
            app.song.tracks[0].blocks.len(),
            MAX_BLOCKS,
            "the song must not outgrow the audio-side mirror"
        );
        assert!(app.status.contains("blocks"), "status: {}", app.status);
    }

    #[test]
    fn placing_a_block_moves_it_and_keeps_the_lane_sorted() {
        let mut app = headless_app();
        for start in [0.0, 8.0, 16.0] {
            app.handle(Event::CreateBlock {
                track: 0,
                start_beats: start,
                length_beats: 4.0,
            });
        }
        let name = app.song.tracks[0].blocks[1].name.clone();
        app.handle(Event::PlaceBlock {
            track: 0,
            block: 1,
            target_track: 0,
            start_beats: -4.0,
        });
        assert_eq!(app.cursor, Cursor { track: 0, block: 1 });
        assert_eq!(app.song.tracks[0].blocks[1].name, name);
        assert_eq!(app.song.tracks[0].blocks[1].start_beats, 0.0);
        assert_eq!(app.song.tracks[0].blocks.len(), 3);
        let starts: Vec<f32> = app.song.tracks[0]
            .blocks
            .iter()
            .map(|block| block.start_beats)
            .collect();
        assert!(starts.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn placing_a_block_moves_it_across_tracks_with_its_source() {
        let mut app = headless_app();
        app.song.tracks[0].insert_block(Block::new("a", TrackKind::Audio, 0.0, 4.0, "/tmp/a.wav"));
        app.song.tracks[0].insert_block(Block::new("b", TrackKind::Audio, 8.0, 4.0, "/tmp/b.wav"));
        app.handle(Event::PlaceBlock {
            track: 0,
            block: 0,
            target_track: 2,
            start_beats: 12.0,
        });
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(app.song.tracks[2].blocks.len(), 1);
        let moved = &app.song.tracks[2].blocks[0];
        assert_eq!(moved.name, "a");
        assert_eq!(moved.source, "/tmp/a.wav");
        assert_eq!(moved.start_beats, 12.0);
        assert_eq!(app.cursor, Cursor { track: 2, block: 0 });
    }

    #[test]
    fn placing_onto_a_track_that_does_not_exist_lands_on_the_last_one() {
        let mut app = headless_app();
        app.song.tracks[0].insert_block(Block::new("a", TrackKind::Midi, 0.0, 4.0, ""));
        app.handle(Event::PlaceBlock {
            track: 0,
            block: 0,
            target_track: 99,
            start_beats: 0.0,
        });
        assert_eq!(app.cursor.track, app.song.tracks.len() - 1);
    }

    #[test]
    fn resizing_clamps_to_the_block_limits() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.handle(Event::ResizeBlock {
            track: 0,
            block: 0,
            length_beats: 0.0,
        });
        assert_eq!(
            app.song.tracks[0].blocks[0].length_beats,
            Block::MIN_LENGTH_BEATS
        );
        app.handle(Event::ResizeBlock {
            track: 0,
            block: 0,
            length_beats: 99_999.0,
        });
        assert_eq!(app.song.tracks[0].blocks[0].length_beats, MAX_LENGTH_BEATS);
    }

    #[test]
    fn resizing_a_block_that_is_not_there_changes_nothing() {
        let mut app = headless_app();
        app.handle(Event::ResizeBlock {
            track: 0,
            block: 3,
            length_beats: 8.0,
        });
        assert!(app.song.tracks[0].blocks.is_empty());
        assert!(app.status.contains("resize"), "status: {}", app.status);
    }

    #[test]
    fn deleting_removes_the_block_under_the_cursor() {
        let mut app = headless_app();
        for _ in 0..2 {
            app.handle(Event::AddBlock);
        }
        app.cursor.block = 0;
        app.handle(Event::DeleteBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(
            app.song.tracks[0].blocks[0].name, "2",
            "the block under the cursor goes, not the last one"
        );
        assert_eq!(app.cursor.block, 0);
    }

    #[test]
    fn deleting_the_last_block_lands_the_cursor_on_the_add_slot() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::DeleteBlock);
        assert!(app.song.tracks[0].blocks.is_empty());
        assert_eq!(app.cursor.block, 0);
    }

    #[test]
    fn deleting_with_the_cursor_on_the_add_slot_changes_nothing() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 1;
        app.handle(Event::DeleteBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
    }

    #[test]
    fn a_deletion_empties_the_lane_mirror() {
        let mut app = headless_app();
        for start in [0.0, 8.0] {
            app.handle(Event::CreateBlock {
                track: 0,
                start_beats: start,
                length_beats: 4.0,
            });
        }
        assert_eq!(app.mirror(0).len(), 2);
        app.handle(Event::DeleteBlockAt { track: 0, block: 0 });
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(
            app.mirror(0).len(),
            1,
            "removal must reach the audio-side mirror"
        );
        assert!(
            !app.resync_pending,
            "a deletion pushes its lane, it no longer needs a full resync"
        );
    }

    #[test]
    fn a_right_click_delete_takes_the_block_it_names() {
        let mut app = headless_app();
        for start in [0.0, 8.0] {
            app.handle(Event::CreateBlock {
                track: 0,
                start_beats: start,
                length_beats: 4.0,
            });
        }
        app.handle(Event::DeleteBlockAt { track: 0, block: 1 });
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(app.song.tracks[0].blocks[0].start_beats, 0.0);
    }

    #[test]
    fn a_midi_block_mirrors_with_no_slot_and_no_gain() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 2.0,
            length_beats: 4.0,
        });
        let mirror = app.mirror(0);
        assert_eq!(mirror.len(), 1);
        assert_eq!(mirror[0].start_beats, 2.0);
        assert_eq!(mirror[0].end_beats, 6.0);
        assert_eq!(mirror[0].slot, NO_SLOT);
        assert_eq!(mirror[0].gain, 0.0);
    }

    #[test]
    fn an_audio_block_mirrors_the_slot_of_its_source() {
        let mut app = headless_app();
        app.samples.insert("/tmp/a.wav".into(), 3);
        app.song.tracks[2].insert_block(Block::new("a", TrackKind::Audio, 0.0, 4.0, "/tmp/a.wav"));
        app.song.tracks[2].insert_block(Block::new(
            "b",
            TrackKind::Audio,
            8.0,
            4.0,
            "/tmp/gone.wav",
        ));
        let mirror = app.mirror(2);
        assert_eq!(mirror[0].slot, 3);
        assert_eq!(
            mirror[1].slot, NO_SLOT,
            "a source that is not loaded is not a slot to guess at"
        );
    }

    fn note(pitch: u8, start_beats: f32, length_beats: f32) -> Note {
        Note {
            pitch,
            start_beats,
            length_beats,
            velocity: 90,
        }
    }

    #[test]
    fn the_note_mirror_is_absolute_sorted_and_clipped_to_each_block() {
        let mut app = headless_app();
        let mut late = Block::new("late", TrackKind::Midi, 16.0, 4.0, "");
        late.notes = vec![note(72, 2.0, 1.0), note(71, 0.0, 1.0)];
        let mut early = Block::new("early", TrackKind::Midi, 4.0, 2.0, "");
        early.notes = vec![note(60, 1.5, 4.0), note(61, 2.0, 1.0), note(62, 0.5, 0.5)];
        let mut audio = Block::new("audio", TrackKind::Audio, 0.0, 4.0, "/tmp/a.wav");
        audio.notes = vec![note(50, 0.0, 1.0)];
        for block in [late, early, audio] {
            app.song.tracks[0].insert_block(block);
        }
        let mirror = app.mirror_notes(0);
        let flat: Vec<(f32, f32, u8)> = mirror
            .iter()
            .map(|n| (n.start_beats, n.end_beats, n.pitch))
            .collect();
        assert_eq!(
            flat,
            vec![
                (4.5, 5.0, 62),
                (5.5, 6.0, 60),
                (16.0, 17.0, 71),
                (18.0, 19.0, 72)
            ],
            "the note at the block end and the audio block's note are dropped"
        );
        assert!(mirror.iter().all(|n| n.velocity == 90));
    }

    #[test]
    fn the_note_mirror_stops_at_its_ceiling() {
        let mut app = headless_app();
        for index in 0..5 {
            let mut block = Block::new("b", TrackKind::Midi, index as f32 * 8.0, 8.0, "");
            block.notes = vec![note(60, 0.0, 1.0); crate::models::MAX_NOTES_PER_BLOCK];
            app.song.tracks[0].insert_block(block);
        }
        assert_eq!(app.mirror_notes(0).len(), MAX_NOTES);
    }

    #[test]
    fn moving_a_block_moves_its_notes_in_the_mirror() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.handle(Event::AddNote {
            track: 0,
            block: 0,
            pitch: 60,
            start_beats: 1.0,
            length_beats: 1.0,
            velocity: 100,
        });
        assert_eq!(app.mirror_notes(0)[0].start_beats, 1.0);
        app.handle(Event::PlaceBlock {
            track: 0,
            block: 0,
            target_track: 1,
            start_beats: 8.0,
        });
        assert!(app.mirror_notes(0).is_empty());
        let moved = app.mirror_notes(1);
        assert_eq!((moved[0].start_beats, moved[0].end_beats), (9.0, 10.0));
        app.handle(Event::ResizeBlock {
            track: 1,
            block: 0,
            length_beats: 1.0,
        });
        assert!(
            app.mirror_notes(1).is_empty(),
            "a shrunk block hides the notes past its end"
        );
        assert_eq!(app.song.tracks[1].blocks[0].notes.len(), 1);
        assert!(!app.resync_pending);
    }

    fn seconds_per_beat(app: &App) -> f32 {
        60.0 / app.song.bpm()
    }

    #[test]
    fn splitting_an_audio_block_keeps_the_right_half_in_step() {
        let mut app = headless_app();
        app.song.tracks[2].insert_block(Block::new("a", TrackKind::Audio, 4.0, 8.0, "/tmp/a.wav"));
        app.handle(Event::SplitBlock {
            track: 2,
            block: 0,
            at_beats: 7.0,
        });
        let lane = &app.song.tracks[2].blocks;
        assert_eq!(lane.len(), 2);
        assert_eq!((lane[0].start_beats, lane[0].length_beats), (4.0, 3.0));
        assert_eq!((lane[1].start_beats, lane[1].length_beats), (7.0, 5.0));
        assert_eq!(lane[0].offset_seconds, 0.0);
        let expected = 3.0 * seconds_per_beat(&app);
        assert!((lane[1].offset_seconds - expected).abs() < 1e-5);
        assert_eq!(lane[1].source, "/tmp/a.wav");
        assert_ne!(lane[0].name, lane[1].name);
        assert_eq!(app.cursor, Cursor { track: 2, block: 1 });
    }

    #[test]
    fn splitting_a_midi_block_hands_over_its_later_notes() {
        let mut app = headless_app();
        let mut block = Block::new("m", TrackKind::Midi, 0.0, 8.0, "");
        block.notes = vec![
            Note {
                pitch: 60,
                start_beats: 1.0,
                length_beats: 4.0,
                velocity: 100,
            },
            Note {
                pitch: 62,
                start_beats: 5.0,
                length_beats: 1.0,
                velocity: 90,
            },
        ];
        app.song.tracks[0].insert_block(block);
        app.handle(Event::SplitBlock {
            track: 0,
            block: 0,
            at_beats: 4.0,
        });
        let lane = &app.song.tracks[0].blocks;
        assert_eq!(lane[0].notes.len(), 1);
        assert_eq!(
            lane[0].notes[0].length_beats, 3.0,
            "the crossing note is cut"
        );
        assert_eq!(lane[1].notes.len(), 1);
        assert_eq!(lane[1].notes[0].pitch, 62);
        assert_eq!(lane[1].notes[0].start_beats, 1.0);
        assert_eq!(lane[1].offset_seconds, 0.0);
    }

    #[test]
    fn a_split_at_or_outside_an_edge_changes_nothing() {
        let mut app = headless_app();
        app.song.tracks[0].insert_block(Block::new("a", TrackKind::Midi, 4.0, 4.0, ""));
        for at_beats in [2.0, 4.0, 4.1, 8.0, 12.0] {
            app.handle(Event::SplitBlock {
                track: 0,
                block: 0,
                at_beats,
            });
        }
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(app.song.tracks[0].blocks[0].length_beats, 4.0);
    }

    #[test]
    fn duplicating_places_a_renamed_copy_right_after() {
        let mut app = headless_app();
        let mut block = Block::new("a", TrackKind::Audio, 2.0, 3.0, "/tmp/a.wav");
        block.offset_seconds = 0.5;
        block.rate = 2.0;
        app.song.tracks[2].insert_block(block);
        app.handle(Event::DuplicateBlock { track: 2, block: 0 });
        let lane = &app.song.tracks[2].blocks;
        assert_eq!(lane.len(), 2);
        assert_eq!(lane[1].start_beats, 5.0);
        assert_eq!(lane[1].length_beats, 3.0);
        assert_eq!((lane[1].offset_seconds, lane[1].rate), (0.5, 2.0));
        assert_ne!(lane[1].name, lane[0].name);
        assert_eq!(app.cursor, Cursor { track: 2, block: 1 });
    }

    #[test]
    fn trimming_the_start_advances_the_offset_and_keeps_the_end() {
        let mut app = headless_app();
        app.song.tracks[2].insert_block(Block::new("a", TrackKind::Audio, 4.0, 8.0, "/tmp/a.wav"));
        app.handle(Event::TrimBlockStart {
            track: 2,
            block: 0,
            start_beats: 6.0,
        });
        let block = &app.song.tracks[2].blocks[0];
        assert_eq!((block.start_beats, block.end_beats()), (6.0, 12.0));
        assert!((block.offset_seconds - 2.0 * seconds_per_beat(&app)).abs() < 1e-5);

        app.handle(Event::TrimBlockStart {
            track: 2,
            block: 0,
            start_beats: 0.0,
        });
        let block = &app.song.tracks[2].blocks[0];
        assert!(
            (block.start_beats - 4.0).abs() < 1e-4,
            "it cannot reach before its material"
        );
        assert!(block.offset_seconds.abs() < 1e-5);
        assert_eq!(block.end_beats(), 12.0);
    }

    #[test]
    fn trimming_a_midi_start_shifts_and_drops_notes() {
        let mut app = headless_app();
        let mut block = Block::new("m", TrackKind::Midi, 0.0, 8.0, "");
        block.notes = vec![
            Note {
                pitch: 60,
                start_beats: 0.0,
                length_beats: 1.0,
                velocity: 100,
            },
            Note {
                pitch: 62,
                start_beats: 1.0,
                length_beats: 2.0,
                velocity: 100,
            },
            Note {
                pitch: 64,
                start_beats: 4.0,
                length_beats: 1.0,
                velocity: 100,
            },
        ];
        app.song.tracks[0].insert_block(block);
        app.handle(Event::TrimBlockStart {
            track: 0,
            block: 0,
            start_beats: 2.0,
        });
        let block = &app.song.tracks[0].blocks[0];
        assert_eq!((block.start_beats, block.length_beats), (2.0, 6.0));
        let notes: Vec<(u8, f32, f32)> = block
            .notes
            .iter()
            .map(|note| (note.pitch, note.start_beats, note.length_beats))
            .collect();
        assert_eq!(notes, vec![(62, 0.0, 1.0), (64, 2.0, 1.0)]);
    }

    #[test]
    fn stretching_changes_the_rate_within_its_limits() {
        let mut app = headless_app();
        app.song.tracks[2].insert_block(Block::new("a", TrackKind::Audio, 0.0, 4.0, "/tmp/a.wav"));
        app.handle(Event::StretchBlock {
            track: 2,
            block: 0,
            length_beats: 8.0,
        });
        let block = &app.song.tracks[2].blocks[0];
        assert_eq!((block.length_beats, block.rate), (8.0, 0.5));
        assert_eq!(app.mirror(2)[0].rate, 0.5);

        app.handle(Event::StretchBlock {
            track: 2,
            block: 0,
            length_beats: 0.25,
        });
        let block = &app.song.tracks[2].blocks[0];
        assert_eq!(block.rate, Block::MAX_RATE);
        assert_eq!(
            block.length_beats, 1.0,
            "the length follows the clamped rate"
        );
    }

    #[test]
    fn stretching_a_midi_block_resizes_it() {
        let mut app = headless_app();
        app.song.tracks[0].insert_block(Block::new("m", TrackKind::Midi, 0.0, 4.0, ""));
        app.handle(Event::StretchBlock {
            track: 0,
            block: 0,
            length_beats: 6.0,
        });
        let block = &app.song.tracks[0].blocks[0];
        assert_eq!((block.length_beats, block.rate), (6.0, 1.0));
    }
}
