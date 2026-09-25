//! Blocks on the timeline: creation, placement, resizing, deletion, and the
//! mirror the audio thread reads.
//!
//! The song owns the truth. The audio side gets a flattened copy of one lane
//! whenever that lane changes, so a drag costs one small `Vec` and the render
//! path never looks at a [`Track`](crate::models::Track).

use super::App;
use crate::core::MAX_BLOCKS;
use crate::core::render::MirroredBlock;
use crate::core::sample::NO_SLOT;
use crate::core::synth::sample_gain;
use crate::models::{Block, TrackKind};
use crate::ui::view::Cursor;

impl App {
    /// Create a block at a position the UI asked for and select it.
    pub(crate) fn create_block(&mut self, track: usize, start_beats: f32, length_beats: f32) {
        let Some(track_index) = self.clamp_track(track) else {
            return;
        };
        if self.song.tracks[track_index].blocks.len() >= MAX_BLOCKS {
            self.status = format!(
                "track {} is already at {MAX_BLOCKS} blocks",
                track_index + 1
            );
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
                    };
                }
                MirroredBlock {
                    start_beats: block.start_beats,
                    end_beats: block.end_beats(),
                    slot: self.samples.get(&block.source).copied().unwrap_or(NO_SLOT),
                    gain: sample_gain(&source.instrument),
                }
            })
            .collect()
    }

    /// Replace one lane's mirror on the audio side. Call it after any edit that
    /// changes a lane.
    pub(crate) fn push_mirror(&mut self, track: usize) {
        let blocks = self.mirror(track);
        if !self.engine.set_blocks(track, blocks) {
            self.resync_pending = true;
        }
    }

    /// Push every lane's mirror, after a resync has replaced them with empty
    /// ones. `AudioEngine::resync` can only see the song, so the sample slots
    /// and the voice gains it cannot know have to come back from here.
    pub(crate) fn repush_lanes(&mut self) {
        for track in 0..self.song.tracks.len() {
            let blocks = self.mirror(track);
            if !self.engine.set_blocks(track, blocks) {
                self.resync_pending = true;
                return;
            }
        }
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
    use crate::models::MAX_LENGTH_BEATS;
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
}
