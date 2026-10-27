//! The arrangement clipboard: copying, cutting, clearing, pasting and
//! duplicating a range of tracks and beats.

use super::App;
use crate::core::MAX_BLOCKS;
use crate::events::BeatRange;
use crate::models::{Block, Song, Track};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Clipboard {
    pub length_beats: f32,
    pub lanes: Vec<Vec<Block>>,
}

fn normalize_range(song: &Song, range: BeatRange) -> Option<(usize, usize, f32, f32)> {
    if song.tracks.is_empty() {
        return None;
    }
    if !range.start_beats.is_finite() || !range.end_beats.is_finite() {
        return None;
    }
    let (t_start, t_end) = if range.first_track <= range.last_track {
        (range.first_track, range.last_track)
    } else {
        (range.last_track, range.first_track)
    };
    if t_start >= song.tracks.len() {
        return None;
    }
    let last_track = t_end.min(song.tracks.len() - 1);
    let (b_start, b_end) = if range.start_beats <= range.end_beats {
        (range.start_beats, range.end_beats)
    } else {
        (range.end_beats, range.start_beats)
    };
    let b_start = b_start.max(0.0);
    let b_end = b_end.max(0.0);
    if b_end <= b_start {
        return None;
    }
    Some((t_start, last_track, b_start, b_end))
}

fn format_beats(beats: f32) -> String {
    if beats.fract() == 0.0 {
        format!("{:.0}", beats)
    } else {
        format!("{beats}")
    }
}

fn split_track_at(track: &mut Track, at_beats: f32, seconds_per_beat: f32) {
    let mut new_blocks = Vec::new();
    for block in &mut track.blocks {
        if block.start_beats < at_beats
            && block.end_beats() > at_beats
            && let Some(right) = block.split_off(at_beats, seconds_per_beat)
        {
            new_blocks.push(right);
        }
    }
    for mut right in new_blocks {
        right.name = track.next_name();
        track.insert_block(right);
    }
}

fn extract_lane(
    track: &Track,
    start_beats: f32,
    end_beats: f32,
    seconds_per_beat: f32,
) -> Vec<Block> {
    let mut lane_blocks = Vec::new();
    for block in &track.blocks {
        if block.end_beats() <= start_beats || block.start_beats >= end_beats {
            continue;
        }
        let mut piece = block.clone();
        if piece.start_beats < start_beats
            && let Some(right) = piece.split_off(start_beats, seconds_per_beat)
        {
            piece = right;
        }
        if piece.end_beats() > end_beats {
            let _ = piece.split_off(end_beats, seconds_per_beat);
        }
        piece.start_beats = (piece.start_beats - start_beats).max(0.0);
        lane_blocks.push(piece);
    }
    lane_blocks
}

impl App {
    pub(crate) fn copy_range(&mut self, range: BeatRange) {
        let Some((first_track, last_track, start_beats, end_beats)) =
            normalize_range(&self.song, range)
        else {
            self.status = "selection is empty".into();
            return;
        };
        let length_beats = end_beats - start_beats;
        let seconds_per_beat = self.seconds_per_beat();
        let mut lanes = Vec::with_capacity(last_track - first_track + 1);
        for track_idx in first_track..=last_track {
            lanes.push(extract_lane(
                &self.song.tracks[track_idx],
                start_beats,
                end_beats,
                seconds_per_beat,
            ));
        }
        self.clipboard = Some(Clipboard {
            length_beats,
            lanes,
        });
        let track_count = last_track - first_track + 1;
        self.status = format!(
            "copied {track_count} track{}, {} beat{}",
            if track_count == 1 { "" } else { "s" },
            format_beats(length_beats),
            if length_beats == 1.0 { "" } else { "s" }
        );
    }

    pub(crate) fn delete_range(&mut self, range: BeatRange) {
        let Some((first_track, last_track, start_beats, end_beats)) =
            normalize_range(&self.song, range)
        else {
            return;
        };
        let length_beats = end_beats - start_beats;
        let seconds_per_beat = self.seconds_per_beat();
        for track_idx in first_track..=last_track {
            split_track_at(
                &mut self.song.tracks[track_idx],
                start_beats,
                seconds_per_beat,
            );
            split_track_at(
                &mut self.song.tracks[track_idx],
                end_beats,
                seconds_per_beat,
            );
            self.song.tracks[track_idx]
                .blocks
                .retain(|b| b.end_beats() <= start_beats || b.start_beats >= end_beats);
            self.push_mirror(track_idx);
            self.push_notes(track_idx);
            self.touch_notes(track_idx);
        }
        self.garbage_collect_samples();
        let track_count = last_track - first_track + 1;
        self.status = format!(
            "deleted {track_count} track{}, {} beat{}",
            if track_count == 1 { "" } else { "s" },
            format_beats(length_beats),
            if length_beats == 1.0 { "" } else { "s" }
        );
    }

    pub(crate) fn cut_range(&mut self, range: BeatRange) {
        let Some((first_track, last_track, start_beats, end_beats)) =
            normalize_range(&self.song, range)
        else {
            self.status = "selection is empty".into();
            return;
        };
        self.copy_range(range);
        self.delete_range(range);
        let track_count = last_track - first_track + 1;
        let length_beats = end_beats - start_beats;
        self.status = format!(
            "cut {track_count} track{}, {} beat{}",
            if track_count == 1 { "" } else { "s" },
            format_beats(length_beats),
            if length_beats == 1.0 { "" } else { "s" }
        );
    }

    pub(crate) fn paste_range(&mut self, track: usize, start_beats: f32) {
        let Some(clipboard) = self.clipboard.clone() else {
            self.status = "clipboard is empty".into();
            return;
        };
        if track >= self.song.tracks.len() {
            self.status = "no track there to paste onto".into();
            return;
        }
        if !start_beats.is_finite() {
            return;
        }
        let start_beats = start_beats.max(0.0);
        let length_beats = clipboard.length_beats;
        let end_beats = start_beats + length_beats;
        let seconds_per_beat = self.seconds_per_beat();

        let total_lanes = clipboard.lanes.len();
        let available_tracks = self.song.tracks.len() - track;
        let pasted_lanes = total_lanes.min(available_tracks);
        let dropped_lanes = total_lanes.saturating_sub(pasted_lanes);

        for (i, lane_blocks) in clipboard.lanes.into_iter().take(pasted_lanes).enumerate() {
            let target_track = track + i;
            split_track_at(
                &mut self.song.tracks[target_track],
                start_beats,
                seconds_per_beat,
            );
            split_track_at(
                &mut self.song.tracks[target_track],
                end_beats,
                seconds_per_beat,
            );
            self.song.tracks[target_track]
                .blocks
                .retain(|b| b.end_beats() <= start_beats || b.start_beats >= end_beats);

            for block_template in lane_blocks {
                if self.song.tracks[target_track].blocks.len() >= MAX_BLOCKS {
                    break;
                }
                let mut block = block_template;
                block.start_beats += start_beats;
                block.name = self.song.tracks[target_track].next_name();
                if !block.source.is_empty() {
                    self.load_sample_source(&block.source);
                }
                self.song.tracks[target_track].insert_block(block);
            }

            self.push_mirror(target_track);
            self.push_notes(target_track);
            self.touch_notes(target_track);
        }

        if dropped_lanes > 0 {
            self.status = format!(
                "pasted {pasted_lanes} track{}, dropped {dropped_lanes} past last track",
                if pasted_lanes == 1 { "" } else { "s" }
            );
        } else {
            self.status = format!(
                "pasted {pasted_lanes} track{}, {} beat{}",
                if pasted_lanes == 1 { "" } else { "s" },
                format_beats(length_beats),
                if length_beats == 1.0 { "" } else { "s" }
            );
        }
    }

    pub(crate) fn duplicate_range(&mut self, range: BeatRange) {
        let Some((first_track, last_track, start_beats, end_beats)) =
            normalize_range(&self.song, range)
        else {
            self.status = "selection is empty".into();
            return;
        };
        let length_beats = end_beats - start_beats;
        let seconds_per_beat = self.seconds_per_beat();

        let mut lanes = Vec::with_capacity(last_track - first_track + 1);
        for track_idx in first_track..=last_track {
            lanes.push(extract_lane(
                &self.song.tracks[track_idx],
                start_beats,
                end_beats,
                seconds_per_beat,
            ));
        }

        let paste_start = end_beats;
        let paste_end = paste_start + length_beats;

        for (i, lane_blocks) in lanes.into_iter().enumerate() {
            let target_track = first_track + i;
            split_track_at(
                &mut self.song.tracks[target_track],
                paste_start,
                seconds_per_beat,
            );
            split_track_at(
                &mut self.song.tracks[target_track],
                paste_end,
                seconds_per_beat,
            );
            self.song.tracks[target_track]
                .blocks
                .retain(|b| b.end_beats() <= paste_start || b.start_beats >= paste_end);

            for block_template in lane_blocks {
                if self.song.tracks[target_track].blocks.len() >= MAX_BLOCKS {
                    break;
                }
                let mut block = block_template;
                block.start_beats += paste_start;
                block.name = self.song.tracks[target_track].next_name();
                if !block.source.is_empty() {
                    self.load_sample_source(&block.source);
                }
                self.song.tracks[target_track].insert_block(block);
            }

            self.push_mirror(target_track);
            self.push_notes(target_track);
            self.touch_notes(target_track);
        }

        let track_count = last_track - first_track + 1;
        self.status = format!(
            "duplicated {track_count} track{}, {} beat{}",
            if track_count == 1 { "" } else { "s" },
            format_beats(length_beats),
            if length_beats == 1.0 { "" } else { "s" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;
    use crate::models::TrackKind;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    #[test]
    fn copy_and_paste_across_tracks_with_edge_splits() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 8.0,
        });
        app.handle(Event::AddNote {
            track: 0,
            block: 0,
            pitch: 60,
            start_beats: 1.0,
            length_beats: 2.0,
            velocity: 100,
        });
        app.handle(Event::AddNote {
            track: 0,
            block: 0,
            pitch: 64,
            start_beats: 3.0,
            length_beats: 3.0,
            velocity: 100,
        });

        app.handle(Event::CreateBlock {
            track: 1,
            start_beats: 2.0,
            length_beats: 4.0,
        });

        let range = BeatRange {
            first_track: 0,
            last_track: 1,
            start_beats: 2.0,
            end_beats: 6.0,
        };
        app.handle(Event::CopyRange { range });
        assert_eq!(app.status, "copied 2 tracks, 4 beats");
        let clip_view = app.ui_state().clipboard.expect("clipboard filled");
        assert_eq!(clip_view.tracks, 2);
        assert_eq!(clip_view.length_beats, 4.0);

        let clip = app.clipboard.as_ref().unwrap();
        assert_eq!(clip.lanes[0].len(), 1);
        assert_eq!(clip.lanes[0][0].start_beats, 0.0);
        assert_eq!(clip.lanes[0][0].length_beats, 4.0);
        assert_eq!(clip.lanes[0][0].notes.len(), 1);
        assert_eq!(clip.lanes[0][0].notes[0].pitch, 64);
        assert_eq!(clip.lanes[0][0].notes[0].start_beats, 1.0);
        assert_eq!(clip.lanes[0][0].notes[0].length_beats, 3.0);

        assert_eq!(app.song.tracks[0].blocks[0].length_beats, 8.0);

        app.handle(Event::PasteRange {
            track: 2,
            start_beats: 10.0,
        });
        assert_eq!(app.song.tracks[2].blocks.len(), 1);
        assert_eq!(app.song.tracks[2].blocks[0].start_beats, 10.0);
        assert_eq!(app.song.tracks[2].blocks[0].length_beats, 4.0);
        assert_eq!(app.song.tracks[3].blocks.len(), 1);
        assert_eq!(app.song.tracks[3].blocks[0].start_beats, 10.0);
        assert_eq!(app.song.tracks[3].blocks[0].length_beats, 4.0);
        assert_eq!(app.status, "pasted 2 tracks, 4 beats");

        app.handle(Event::Undo);
        assert!(app.song.tracks[2].blocks.is_empty());
        assert!(app.song.tracks[3].blocks.is_empty());
    }

    #[test]
    fn delete_range_splits_at_both_edges_without_shifting_time() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 8.0,
        });
        app.handle(Event::AddNote {
            track: 0,
            block: 0,
            pitch: 60,
            start_beats: 1.0,
            length_beats: 1.0,
            velocity: 100,
        });
        app.handle(Event::AddNote {
            track: 0,
            block: 0,
            pitch: 72,
            start_beats: 6.5,
            length_beats: 1.0,
            velocity: 100,
        });

        app.handle(Event::DeleteRange {
            range: BeatRange {
                first_track: 0,
                last_track: 0,
                start_beats: 2.0,
                end_beats: 6.0,
            },
        });
        assert_eq!(app.status, "deleted 1 track, 4 beats");
        assert_eq!(app.song.tracks[0].blocks.len(), 2);
        assert_eq!(app.song.tracks[0].blocks[0].start_beats, 0.0);
        assert_eq!(app.song.tracks[0].blocks[0].length_beats, 2.0);
        assert_eq!(app.song.tracks[0].blocks[0].notes.len(), 1);
        assert_eq!(app.song.tracks[0].blocks[0].notes[0].pitch, 60);

        assert_eq!(app.song.tracks[0].blocks[1].start_beats, 6.0);
        assert_eq!(app.song.tracks[0].blocks[1].length_beats, 2.0);
        assert_eq!(app.song.tracks[0].blocks[1].notes.len(), 1);
        assert_eq!(app.song.tracks[0].blocks[1].notes[0].pitch, 72);
        assert_eq!(app.song.tracks[0].blocks[1].notes[0].start_beats, 0.5);

        app.handle(Event::Undo);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(app.song.tracks[0].blocks[0].length_beats, 8.0);
    }

    #[test]
    fn cut_range_copies_then_deletes() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 8.0,
        });
        app.handle(Event::CutRange {
            range: BeatRange {
                first_track: 0,
                last_track: 0,
                start_beats: 2.0,
                end_beats: 6.0,
            },
        });
        assert_eq!(app.status, "cut 1 track, 4 beats");
        assert!(app.clipboard.is_some());
        assert_eq!(app.song.tracks[0].blocks.len(), 2);
    }

    #[test]
    fn duplicate_range_pastes_after_and_leaves_clipboard_untouched() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.clipboard = None;

        app.handle(Event::DuplicateRange {
            range: BeatRange {
                first_track: 0,
                last_track: 0,
                start_beats: 0.0,
                end_beats: 4.0,
            },
        });
        assert_eq!(app.song.tracks[0].blocks.len(), 2);
        assert_eq!(app.song.tracks[0].blocks[0].start_beats, 0.0);
        assert_eq!(app.song.tracks[0].blocks[1].start_beats, 4.0);
        assert_eq!(app.song.tracks[0].blocks[1].length_beats, 4.0);
        assert!(app.clipboard.is_none());
        assert_eq!(app.status, "duplicated 1 track, 4 beats");

        app.handle(Event::Undo);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
    }

    #[test]
    fn paste_drops_lanes_past_last_track() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.handle(Event::CreateBlock {
            track: 1,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.handle(Event::CopyRange {
            range: BeatRange {
                first_track: 0,
                last_track: 1,
                start_beats: 0.0,
                end_beats: 4.0,
            },
        });
        app.handle(Event::PasteRange {
            track: 3,
            start_beats: 0.0,
        });
        assert_eq!(app.status, "pasted 1 track, dropped 1 past last track");
    }

    #[test]
    fn audio_block_split_at_edges_updates_offset_seconds() {
        let mut app = headless_app();
        let mut audio_block = Block::new("audio", TrackKind::Audio, 0.0, 10.0, "test.wav");
        audio_block.offset_seconds = 1.0;
        audio_block.rate = 1.5;
        app.song.tracks[2].insert_block(audio_block);

        let sec_per_beat = app.seconds_per_beat();
        app.handle(Event::CopyRange {
            range: BeatRange {
                first_track: 2,
                last_track: 2,
                start_beats: 2.0,
                end_beats: 6.0,
            },
        });
        let clip = app.clipboard.expect("clipboard exists");
        let copied = &clip.lanes[0][0];
        assert_eq!(copied.start_beats, 0.0);
        assert_eq!(copied.length_beats, 4.0);
        assert_eq!(copied.rate, 1.5);
        let expected_offset = 1.0 + 2.0 * sec_per_beat * 1.5;
        assert!((copied.offset_seconds - expected_offset).abs() < 1e-4);
    }
}
