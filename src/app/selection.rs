//! Multi-block selection: a set of blocks kept next to the cursor, and the
//! edits that act on the whole set at once.
//!
//! With no explicit set, the selection is the block under the cursor, so every
//! single-block path keeps working. A set exists only while it holds zero or at
//! least two blocks: one block collapses back onto the cursor. Any edit that
//! shifts blocks or tracks under the set clears it, so it never names the wrong
//! block.

use std::collections::BTreeSet;

use super::App;
use super::clipboard::Clipboard;
use crate::core::MAX_BLOCKS;
use crate::events::Event;
use crate::ui::view::Cursor;

/// A block by its track and its index on the lane.
pub(crate) type BlockRef = (usize, usize);

/// What the selection's indices depend on: the cursor, each track's name and
/// each block's name and start. Equal layouts mean every index still names
/// the same block.
#[derive(Debug, PartialEq)]
pub(crate) struct Layout {
    cursor: Cursor,
    lanes: Vec<(String, Vec<(String, u32)>)>,
}

/// Events that set the selection themselves, so the layout check leaves it be.
fn manages_selection(event: &Event) -> bool {
    matches!(
        event,
        Event::ToggleBlockSelection { .. }
            | Event::SelectBlockRange { .. }
            | Event::SelectAllBlocks
            | Event::SelectBlocksInRange { .. }
            | Event::DeleteSelectedBlocks
            | Event::CopySelectedBlocks
            | Event::DuplicateSelectedBlocks
            | Event::MoveSelectedBlocks { .. }
            | Event::Undo
            | Event::Redo
    )
}

/// Record a block inserted at `index` on `track`, shifting the ones already
/// recorded on that lane at or past it.
fn record_insert(placed: &mut Vec<BlockRef>, track: usize, index: usize) {
    for entry in placed.iter_mut() {
        if entry.0 == track && entry.1 >= index {
            entry.1 += 1;
        }
    }
    placed.push((track, index));
}

/// `"1 block"` or `"3 blocks"`.
fn blocks_label(count: usize) -> String {
    format!("{count} block{}", if count == 1 { "" } else { "s" })
}

impl App {
    /// Whether a block is selected: in the set when there is one, else under
    /// the cursor.
    pub(crate) fn is_selected(&self, track: usize, block: usize) -> bool {
        match &self.selection {
            Some(set) => set.contains(&(track, block)),
            None => self.cursor.track == track && self.cursor.block == block,
        }
    }

    /// The selected blocks that exist, in track then lane order.
    pub(crate) fn selected_blocks(&self) -> Vec<BlockRef> {
        let exists = |entry: &BlockRef| self.block_exists(entry.0, entry.1);
        match &self.selection {
            Some(set) => set.iter().copied().filter(exists).collect(),
            None => std::iter::once((self.cursor.track, self.cursor.block))
                .filter(exists)
                .collect(),
        }
    }

    fn block_exists(&self, track: usize, block: usize) -> bool {
        self.song
            .tracks
            .get(track)
            .is_some_and(|lane| block < lane.blocks.len())
    }

    /// Take a new set, collapsing a single block onto the cursor.
    fn set_selection(&mut self, set: BTreeSet<BlockRef>) {
        if set.len() == 1 {
            let (track, block) = set.into_iter().next().unwrap_or_default();
            self.cursor = Cursor { track, block };
            self.selection = None;
        } else {
            self.selection = Some(set);
        }
    }

    /// Blocks on a run of tracks that overlap a span of beats.
    fn blocks_in(
        &self,
        first_track: usize,
        last_track: usize,
        start_beats: f32,
        end_beats: f32,
    ) -> BTreeSet<BlockRef> {
        let mut found = BTreeSet::new();
        for (track, lane) in self.song.tracks.iter().enumerate() {
            if track < first_track || track > last_track {
                continue;
            }
            for (index, block) in lane.blocks.iter().enumerate() {
                if block.start_beats < end_beats && block.end_beats() > start_beats {
                    found.insert((track, index));
                }
            }
        }
        found
    }

    /// Add a block to the selection, or take it out. The cursor follows the
    /// click, so it anchors the next range selection.
    pub(crate) fn toggle_block_selection(&mut self, track: usize, block: usize) {
        if !self.block_exists(track, block) {
            return;
        }
        let mut set: BTreeSet<BlockRef> = self.selected_blocks().into_iter().collect();
        if !set.remove(&(track, block)) {
            set.insert((track, block));
        }
        self.cursor = Cursor { track, block };
        let count = set.len();
        self.set_selection(set);
        self.status = format!("{} selected", blocks_label(count));
    }

    /// Select from the cursor to a block: on one track the blocks between them
    /// by beats, across tracks every block in the rectangle they span. The
    /// cursor stays put as the anchor.
    pub(crate) fn select_block_range(&mut self, track: usize, block: usize) {
        if !self.block_exists(track, block) {
            return;
        }
        let anchor = (self.cursor.track, self.cursor.block);
        if !self.block_exists(anchor.0, anchor.1) {
            self.select_block(track, block);
            return;
        }
        let from = &self.song.tracks[anchor.0].blocks[anchor.1];
        let to = &self.song.tracks[track].blocks[block];
        let start_beats = from.start_beats.min(to.start_beats);
        let end_beats = from.end_beats().max(to.end_beats());
        let mut set = self.blocks_in(
            anchor.0.min(track),
            anchor.0.max(track),
            start_beats,
            end_beats,
        );
        set.insert(anchor);
        set.insert((track, block));
        let count = set.len();
        self.set_selection(set);
        self.cursor = Cursor {
            track: anchor.0,
            block: anchor.1,
        };
        self.status = format!("{} selected", blocks_label(count));
    }

    /// Select every block of the song.
    pub(crate) fn select_all_blocks(&mut self) {
        let set = self.blocks_in(0, usize::MAX, f32::NEG_INFINITY, f32::INFINITY);
        let count = set.len();
        self.set_selection(set);
        self.status = format!("{} selected", blocks_label(count));
    }

    /// Select the blocks overlapping a rectangle of tracks and beats, in
    /// either order.
    pub(crate) fn select_blocks_in_range(
        &mut self,
        first_track: usize,
        last_track: usize,
        start_beats: f32,
        end_beats: f32,
    ) {
        if !start_beats.is_finite() || !end_beats.is_finite() {
            return;
        }
        let set = self.blocks_in(
            first_track.min(last_track),
            first_track.max(last_track),
            start_beats.min(end_beats),
            start_beats.max(end_beats),
        );
        let count = set.len();
        self.set_selection(set);
        self.status = format!("{} selected", blocks_label(count));
    }

    /// Remove every selected block as one edit.
    pub(crate) fn delete_selected_blocks(&mut self) {
        let picked = self.selected_blocks();
        if picked.is_empty() {
            self.status = "select blocks to delete them".into();
            return;
        }
        let mut touched = BTreeSet::new();
        for &(track, block) in picked.iter().rev() {
            self.song.tracks[track].blocks.remove(block);
            touched.insert(track);
        }
        self.selection = None;
        self.clamp_block();
        for track in touched {
            self.push_mirror(track);
        }
        self.garbage_collect_samples();
        self.status = format!("deleted {}", blocks_label(picked.len()));
    }

    /// Copy the selected blocks to the clipboard, one lane per track from the
    /// first selected track to the last, timed from the earliest start.
    pub(crate) fn copy_selected_blocks(&mut self) {
        let picked = self.selected_blocks();
        let (Some(&(first_track, _)), Some(&(last_track, _))) = (picked.first(), picked.last())
        else {
            self.status = "select blocks to copy them".into();
            return;
        };
        let (start_beats, end_beats) = self.group_span(&picked);
        let mut lanes = vec![Vec::new(); last_track - first_track + 1];
        for &(track, block) in &picked {
            let mut copy = self.song.tracks[track].blocks[block].clone();
            copy.start_beats -= start_beats;
            lanes[track - first_track].push(copy);
        }
        self.clipboard = Some(Clipboard {
            length_beats: end_beats - start_beats,
            lanes,
        });
        self.status = format!("copied {}", blocks_label(picked.len()));
    }

    /// Copy the selected blocks to just after the group's end, keeping their
    /// tracks and spacing, and select the copies.
    pub(crate) fn duplicate_selected_blocks(&mut self) {
        let picked = self.selected_blocks();
        if picked.is_empty() {
            self.status = "select blocks to duplicate them".into();
            return;
        }
        let (start_beats, end_beats) = self.group_span(&picked);
        if let Some(track) = self.overfull_track(&picked, false, |&(track, _)| Some(track)) {
            self.status = format!("track {} is already at {MAX_BLOCKS} blocks", track + 1);
            return;
        }
        let offset = end_beats - start_beats;
        let mut placed = Vec::with_capacity(picked.len());
        for &(track, block) in &picked {
            let lane = &mut self.song.tracks[track];
            let mut copy = lane.blocks[block].clone();
            copy.name = lane.next_name();
            copy.set_start(copy.start_beats + offset);
            let index = lane.insert_block(copy);
            record_insert(&mut placed, track, index);
        }
        let (track, block) = placed[0];
        self.cursor = Cursor { track, block };
        self.push_lanes(&placed);
        self.set_selection(placed.iter().copied().collect());
        self.status = format!("duplicated {}", blocks_label(picked.len()));
    }

    /// Move every selected block by the same beats and tracks. The whole move
    /// is refused when any block would leave the song's tracks or land on a
    /// track of the other kind; a move left of the top of the song stops there.
    pub(crate) fn move_selected_blocks(&mut self, delta_beats: f32, delta_tracks: i32) {
        let picked = self.selected_blocks();
        if picked.is_empty() || !delta_beats.is_finite() {
            return;
        }
        let (start_beats, _) = self.group_span(&picked);
        let delta_beats = delta_beats.max(-start_beats);
        let track_count = self.song.tracks.len() as i64;
        let target = |track: usize| {
            let moved = track as i64 + i64::from(delta_tracks);
            (0..track_count).contains(&moved).then_some(moved as usize)
        };
        for &(track, block) in &picked {
            let Some(to) = target(track) else {
                self.status = "the blocks would leave the tracks".into();
                return;
            };
            if to != track
                && self.song.tracks[to].kind != self.song.tracks[track].blocks[block].kind
            {
                self.status = "a block cannot land on a track of the other kind".into();
                return;
            }
        }
        if delta_tracks != 0
            && let Some(track) = self.overfull_track(&picked, true, |&(track, _)| target(track))
        {
            self.status = format!("track {} is already at {MAX_BLOCKS} blocks", track + 1);
            return;
        }
        if delta_beats == 0.0 && delta_tracks == 0 {
            return;
        }
        let cursor_at = picked
            .iter()
            .position(|&(track, block)| track == self.cursor.track && block == self.cursor.block)
            .unwrap_or(0);
        let mut moved = Vec::with_capacity(picked.len());
        for &(track, block) in picked.iter().rev() {
            moved.push(self.song.tracks[track].blocks.remove(block));
        }
        moved.reverse();
        let mut placed = Vec::with_capacity(picked.len());
        for (&(track, _), mut block) in picked.iter().zip(moved) {
            let to = target(track).unwrap_or(track);
            block.set_start(block.start_beats + delta_beats);
            let index = self.song.tracks[to].insert_block(block);
            record_insert(&mut placed, to, index);
        }
        let (track, block) = placed[cursor_at];
        self.push_lanes(&picked);
        self.push_lanes(&placed);
        self.set_selection(placed.iter().copied().collect());
        self.cursor = Cursor { track, block };
        self.status = format!("moved {}", blocks_label(picked.len()));
    }

    /// The earliest start and the latest end of a group of blocks.
    fn group_span(&self, picked: &[BlockRef]) -> (f32, f32) {
        picked.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(start, end), &(track, block)| {
                let entry = &self.song.tracks[track].blocks[block];
                (start.min(entry.start_beats), end.max(entry.end_beats()))
            },
        )
    }

    /// The first track that would pass [`MAX_BLOCKS`] once every picked block
    /// lands on the track `lands` names for it, net of the picked blocks
    /// `leaving` their own tracks.
    fn overfull_track(
        &self,
        picked: &[BlockRef],
        leaving: bool,
        lands: impl Fn(&BlockRef) -> Option<usize>,
    ) -> Option<usize> {
        let mut counts: Vec<usize> = self.song.tracks.iter().map(|t| t.blocks.len()).collect();
        if leaving {
            for &(track, _) in picked {
                counts[track] -= 1;
            }
        }
        for entry in picked {
            if let Some(track) = lands(entry) {
                counts[track] += 1;
            }
        }
        counts.iter().position(|&count| count > MAX_BLOCKS)
    }

    /// Push the mirror of every track a group of blocks touches, once each.
    fn push_lanes(&mut self, blocks: &[BlockRef]) {
        let tracks: BTreeSet<usize> = blocks.iter().map(|&(track, _)| track).collect();
        for track in tracks {
            self.push_mirror(track);
        }
    }

    /// The layout the selection's indices rely on, when an event may disturb
    /// them; `None` when there is no set or the event manages it.
    pub(crate) fn selection_layout(&self, event: &Event) -> Option<Layout> {
        if self.selection.is_none() || manages_selection(event) {
            return None;
        }
        Some(self.layout())
    }

    fn layout(&self) -> Layout {
        Layout {
            cursor: self.cursor,
            lanes: self
                .song
                .tracks
                .iter()
                .map(|track| {
                    let blocks = track
                        .blocks
                        .iter()
                        .map(|block| (block.name.clone(), block.start_beats.to_bits()))
                        .collect();
                    (track.name.clone(), blocks)
                })
                .collect(),
        }
    }

    /// Drop the set when an event moved blocks, tracks or the cursor under it,
    /// or opened the name prompt, which edits only the cursor's block.
    pub(crate) fn settle_selection(&mut self, before: Option<Layout>) {
        let Some(before) = before else {
            return;
        };
        if self.rename.is_some() || self.layout() != before {
            self.selection = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Block, TrackKind};

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    fn create(app: &mut App, track: usize, start_beats: f32) {
        app.handle(Event::CreateBlock {
            track,
            start_beats,
            length_beats: 4.0,
        });
    }

    fn selected(app: &App) -> Vec<BlockRef> {
        app.ui_state()
            .tracks
            .iter()
            .enumerate()
            .flat_map(|(track, view)| {
                view.blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, block)| block.selected)
                    .map(move |(block, _)| (track, block))
            })
            .collect()
    }

    fn starts(app: &App, track: usize) -> Vec<f32> {
        app.song.tracks[track]
            .blocks
            .iter()
            .map(|block| block.start_beats)
            .collect()
    }

    #[test]
    fn toggling_adds_and_removes_blocks() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 1, 8.0);
        app.handle(Event::SelectBlock { track: 0, block: 0 });
        app.handle(Event::ToggleBlockSelection { track: 1, block: 0 });
        assert_eq!(selected(&app), vec![(0, 0), (1, 0)]);
        app.handle(Event::ToggleBlockSelection { track: 0, block: 0 });
        assert_eq!(selected(&app), vec![(1, 0)]);
        assert_eq!(app.cursor, Cursor { track: 1, block: 0 });
        app.handle(Event::ToggleBlockSelection { track: 1, block: 0 });
        assert!(selected(&app).is_empty());
    }

    #[test]
    fn a_plain_select_resets_the_set_to_one_block() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 0, 8.0);
        app.handle(Event::SelectAllBlocks);
        assert_eq!(selected(&app).len(), 2);
        app.handle(Event::SelectBlock { track: 0, block: 1 });
        assert_eq!(selected(&app), vec![(0, 1)]);
    }

    #[test]
    fn a_range_on_one_track_takes_the_blocks_between() {
        let mut app = headless_app();
        for start in [0.0, 8.0, 16.0, 24.0] {
            create(&mut app, 0, start);
        }
        create(&mut app, 1, 8.0);
        app.handle(Event::SelectBlock { track: 0, block: 1 });
        app.handle(Event::SelectBlockRange { track: 0, block: 3 });
        assert_eq!(selected(&app), vec![(0, 1), (0, 2), (0, 3)]);
        assert_eq!(app.cursor, Cursor { track: 0, block: 1 });
    }

    #[test]
    fn a_range_across_tracks_takes_the_rectangle() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 0, 32.0);
        create(&mut app, 1, 4.0);
        create(&mut app, 1, 40.0);
        create(&mut app, 3, 8.0);
        app.handle(Event::SelectBlock { track: 0, block: 0 });
        app.handle(Event::SelectBlockRange { track: 1, block: 0 });
        assert_eq!(selected(&app), vec![(0, 0), (1, 0)]);
    }

    #[test]
    fn a_beat_range_selects_the_blocks_it_overlaps() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 1, 6.0);
        create(&mut app, 2, 4.0);
        create(&mut app, 3, 3.0);
        app.handle(Event::SelectBlocksInRange {
            first_track: 2,
            last_track: 0,
            start_beats: 7.0,
            end_beats: 3.0,
        });
        assert_eq!(selected(&app), vec![(0, 0), (1, 0), (2, 0)]);
    }

    #[test]
    fn deleting_the_group_is_one_undo_step() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 0, 8.0);
        create(&mut app, 1, 0.0);
        app.handle(Event::SelectAllBlocks);
        app.handle(Event::DeleteSelectedBlocks);
        assert!(app.song.tracks.iter().all(|t| t.blocks.is_empty()));
        app.handle(Event::Undo);
        assert_eq!(starts(&app, 0), vec![0.0, 8.0]);
        assert_eq!(starts(&app, 1), vec![0.0]);
        assert_eq!(selected(&app).len(), 3, "undo brings the selection back");
    }

    #[test]
    fn copying_the_group_fills_one_lane_per_track() {
        let mut app = headless_app();
        create(&mut app, 0, 4.0);
        create(&mut app, 2, 10.0);
        app.handle(Event::SelectAllBlocks);
        app.handle(Event::CopySelectedBlocks);
        let clip = app.clipboard.as_ref().expect("clipboard filled");
        assert_eq!(clip.length_beats, 10.0);
        assert_eq!(clip.lanes.len(), 3);
        assert_eq!(clip.lanes[0][0].start_beats, 0.0);
        assert!(clip.lanes[1].is_empty());
        assert_eq!(clip.lanes[2][0].start_beats, 6.0);
        assert_eq!(selected(&app).len(), 2);
    }

    #[test]
    fn duplicating_the_group_places_copies_after_its_end() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 1, 2.0);
        app.handle(Event::SelectAllBlocks);
        app.handle(Event::DuplicateSelectedBlocks);
        assert_eq!(starts(&app, 0), vec![0.0, 6.0]);
        assert_eq!(starts(&app, 1), vec![2.0, 8.0]);
        assert_eq!(selected(&app), vec![(0, 1), (1, 1)]);
        app.handle(Event::Undo);
        assert_eq!(starts(&app, 0), vec![0.0]);
        assert_eq!(starts(&app, 1), vec![2.0]);
    }

    #[test]
    fn moving_the_group_shifts_beats_and_tracks() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 0, 8.0);
        create(&mut app, 1, 4.0);
        create(&mut app, 0, 20.0);
        app.handle(Event::SelectBlock { track: 0, block: 0 });
        app.handle(Event::ToggleBlockSelection { track: 0, block: 1 });
        app.handle(Event::ToggleBlockSelection { track: 1, block: 0 });
        app.handle(Event::MoveSelectedBlocks {
            delta_beats: 16.0,
            delta_tracks: 0,
        });
        assert_eq!(starts(&app, 0), vec![16.0, 20.0, 24.0]);
        assert_eq!(starts(&app, 1), vec![20.0]);
        assert_eq!(selected(&app), vec![(0, 0), (0, 2), (1, 0)]);
        app.handle(Event::MoveSelectedBlocks {
            delta_beats: -100.0,
            delta_tracks: 0,
        });
        assert_eq!(starts(&app, 0), vec![0.0, 8.0, 20.0], "stops at the top");
        app.handle(Event::Undo);
        app.handle(Event::Undo);
        assert_eq!(starts(&app, 0), vec![0.0, 8.0, 20.0]);
        assert_eq!(starts(&app, 1), vec![4.0]);
    }

    #[test]
    fn a_move_onto_the_wrong_kind_or_off_the_tracks_is_refused_whole() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 1, 0.0);
        app.handle(Event::SelectAllBlocks);
        let revision = app.edit_revision;
        app.handle(Event::MoveSelectedBlocks {
            delta_beats: 4.0,
            delta_tracks: 1,
        });
        assert_eq!(starts(&app, 0), vec![0.0]);
        assert_eq!(starts(&app, 1), vec![0.0]);
        assert!(app.song.tracks[2].blocks.is_empty());
        app.handle(Event::MoveSelectedBlocks {
            delta_beats: 4.0,
            delta_tracks: -1,
        });
        assert_eq!(starts(&app, 0), vec![0.0]);
        assert_eq!(
            app.edit_revision, revision,
            "a refused move is no undo step"
        );
    }

    #[test]
    fn a_move_across_tracks_of_the_same_kind_lands() {
        let mut app = headless_app();
        app.song.tracks[2].insert_block(Block::new("a", TrackKind::Audio, 0.0, 4.0, ""));
        app.handle(Event::SelectBlock { track: 2, block: 0 });
        app.handle(Event::MoveSelectedBlocks {
            delta_beats: 2.0,
            delta_tracks: 1,
        });
        assert!(app.song.tracks[2].blocks.is_empty());
        assert_eq!(starts(&app, 3), vec![2.0]);
        assert_eq!(app.cursor, Cursor { track: 3, block: 0 });
    }

    #[test]
    fn an_edit_that_shifts_blocks_clears_the_set() {
        let mut app = headless_app();
        create(&mut app, 0, 8.0);
        create(&mut app, 0, 16.0);
        app.handle(Event::SelectAllBlocks);
        app.handle(Event::ResizeBlock {
            track: 0,
            block: 0,
            length_beats: 2.0,
        });
        assert_eq!(selected(&app).len(), 2, "a resize keeps every index");
        create(&mut app, 0, 0.0);
        assert_eq!(selected(&app), vec![(0, 0)], "the new block shifted them");
    }

    #[test]
    fn track_edits_clear_the_set() {
        let mut app = headless_app();
        create(&mut app, 0, 0.0);
        create(&mut app, 1, 0.0);
        app.handle(Event::SelectAllBlocks);
        app.handle(Event::ReorderTrack { from: 0, to: 1 });
        assert!(app.selection.is_none());
        app.handle(Event::SelectAllBlocks);
        app.handle(Event::DeleteTrack { track: 0 });
        assert!(app.selection.is_none());
    }

    #[test]
    fn the_wire_names_are_camel_case() {
        let event: Event = serde_json::from_str(
            r#"{"selectBlocksInRange":{"firstTrack":0,"lastTrack":2,"startBeats":1,"endBeats":3}}"#,
        )
        .expect("range selection parses");
        assert_eq!(
            event,
            Event::SelectBlocksInRange {
                first_track: 0,
                last_track: 2,
                start_beats: 1.0,
                end_beats: 3.0,
            }
        );
        let event: Event =
            serde_json::from_str(r#"{"moveSelectedBlocks":{"deltaBeats":-2,"deltaTracks":1}}"#)
                .expect("group move parses");
        assert_eq!(
            event,
            Event::MoveSelectedBlocks {
                delta_beats: -2.0,
                delta_tracks: 1,
            }
        );
        let event: Event = serde_json::from_str(r#""selectAllBlocks""#).expect("unit parses");
        assert_eq!(event, Event::SelectAllBlocks);
    }

    #[test]
    fn record_insert_shifts_later_entries_on_the_same_lane() {
        let mut placed = vec![(0, 0), (0, 1), (1, 0)];
        record_insert(&mut placed, 0, 1);
        assert_eq!(placed, vec![(0, 0), (0, 2), (1, 0), (0, 1)]);
    }
}
