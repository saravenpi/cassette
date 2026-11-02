//! Consolidate: fold a run of blocks on each track into one block, the way
//! Ableton's Ctrl+J does.
//!
//! A MIDI lane merges the notes its blocks let sound into one block. An audio
//! lane is rendered offline before its insert effects and mixer strip, so the
//! clip gain, fades, warp, rate and offsets are baked in while the effects
//! stay live. Gaps between the blocks become silence.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};

use super::App;
use super::export::{EXPORT_SAMPLE_RATE, file_safe, write_atomic};
use crate::core::render::{RenderCore, RtCommand};
use crate::core::wav;
use crate::events::TrackBlock;
use crate::models::{
    Block, Instrument, MAX_LENGTH_BEATS, MAX_NOTES_PER_BLOCK, Note, Song, Track, TrackKind,
};
use crate::ui::view::Cursor;

/// Folder, next to the project file, that consolidated audio is written to.
const CONSOLIDATED_DIR: &str = "Consolidated";
/// Suffix a consolidated block's name carries.
const SUFFIX: &str = " (cons.)";

impl App {
    /// Replace the named blocks of each track with one block spanning them.
    ///
    /// Every replacement is built before the song changes, so a failed render
    /// leaves every lane as it was.
    pub(crate) fn consolidate_blocks(&mut self, blocks: &[TrackBlock]) {
        let groups = self.group_by_track(blocks);
        if groups.is_empty() {
            self.status = "select blocks to consolidate".into();
            return;
        }
        let mut replacements = Vec::with_capacity(groups.len());
        for (track, indices) in groups {
            let built = match self.song.tracks[track].kind {
                TrackKind::Midi => merged_midi(&self.song.tracks[track], &indices),
                TrackKind::Audio => self.rendered_audio(track, &indices),
            };
            match built {
                Ok(block) => replacements.push((track, indices, block)),
                Err(error) => {
                    self.report_export_progress(None);
                    self.status = format!("consolidate failed: {error:#}");
                    return;
                }
            }
        }
        self.report_export_progress(None);
        let count = replacements.len();
        for (track, indices, block) in replacements {
            self.replace_blocks(track, &indices, block);
        }
        self.status = format!(
            "consolidated {count} track{}",
            if count == 1 { "" } else { "s" }
        );
    }

    /// The named blocks that exist, grouped by track, each group sorted and
    /// without repeats.
    fn group_by_track(&self, blocks: &[TrackBlock]) -> BTreeMap<usize, Vec<usize>> {
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for at in blocks {
            let exists = self
                .song
                .tracks
                .get(at.track)
                .is_some_and(|track| at.block < track.blocks.len());
            if exists {
                groups.entry(at.track).or_default().push(at.block);
            }
        }
        for indices in groups.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }
        groups
    }

    /// Swap a lane's blocks at `indices` for `block` and put the cursor on it.
    fn replace_blocks(&mut self, track: usize, indices: &[usize], block: Block) {
        let lane = &mut self.song.tracks[track];
        for &index in indices.iter().rev() {
            lane.blocks.remove(index);
        }
        let index = lane.insert_block(block);
        self.cursor = Cursor {
            track,
            block: index,
        };
        self.push_mirror(track);
    }

    /// Render an audio lane's blocks between the first start and the last end
    /// to a WAV file, load it, and hand back an unwarped block that plays it.
    fn rendered_audio(&mut self, track: usize, indices: &[usize]) -> Result<Block> {
        let lane = &self.song.tracks[track];
        let (start, end) = span(lane, indices)?;
        let frames_per_beat = f64::from(EXPORT_SAMPLE_RATE) * 60.0 / f64::from(self.song.bpm());
        let total_frames = (f64::from(end - start) * frames_per_beat).round() as usize;
        if total_frames > wav::MAX_DECODED_FRAMES {
            bail!("the span is too long to render");
        }
        let name = consolidated_name(&lane.blocks[indices[0]].name);
        let track_name = lane.name.clone();
        let mirror = self.mirror(track);
        let blocks: Vec<_> = indices.iter().map(|&index| mirror[index]).collect();

        let mut core = RenderCore::new(&self.dry_song(), EXPORT_SAMPLE_RATE);
        self.load_samples_into(&mut core);
        core.set_blocks(0, blocks);
        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::Seek { beats: start });
        core.apply(RtCommand::Play);
        let stereo = self.render_frames(&mut core, total_frames);
        let mono: Vec<f32> = stereo
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[left, _]| left * 2.0)
            .collect();

        let path = self.consolidated_path(&track_name);
        write_atomic(&path, |temp| {
            wav::encode_pcm16(temp, &mono, 1, EXPORT_SAMPLE_RATE)
        })?;
        let source = path.to_string_lossy().into_owned();
        self.load_sample_source(&source)
            .ok_or_else(|| anyhow!("cannot load {source}"))?;
        Ok(Block::new(
            name,
            TrackKind::Audio,
            start,
            end - start,
            source,
        ))
    }

    /// A one-track song at this song's tempo with a unity, centred strip, no
    /// effects and a unity master, so a render hears the clips alone.
    fn dry_song(&self) -> Song {
        let mut track = Track::new(
            "dry",
            TrackKind::Audio,
            Instrument {
                enabled: true,
                gain: 1.0,
                ..TrackKind::Audio.default_preset()
            },
        );
        track.volume = 1.0;
        let mut song = Song::new();
        song.set_bpm(self.song.bpm());
        song.tracks = vec![track];
        song.master_volume = 1.0;
        song
    }

    /// The first free `<track name>-<n>.wav` in the project's consolidate
    /// folder, or in the system temp folder while the project is unsaved.
    fn consolidated_path(&self, track_name: &str) -> PathBuf {
        let base = self
            .project_path
            .as_deref()
            .and_then(Path::parent)
            .map_or_else(std::env::temp_dir, Path::to_path_buf)
            .join(CONSOLIDATED_DIR);
        let stem = match file_safe(track_name) {
            name if name.is_empty() => "track".to_string(),
            name => name,
        };
        (1u32..)
            .map(|n| base.join(format!("{stem}-{n}.wav")))
            .find(|path| !path.exists() && !self.samples.contains_key(&*path.to_string_lossy()))
            .expect("an unbounded counter always finds a free name")
    }
}

/// The first start and last end of a lane's blocks at `indices`, refused when
/// longer than a block may be.
fn span(lane: &Track, indices: &[usize]) -> Result<(f32, f32)> {
    let blocks = || indices.iter().map(|&index| &lane.blocks[index]);
    let start = blocks()
        .map(|block| block.start_beats)
        .fold(f32::INFINITY, f32::min);
    let end = blocks()
        .map(Block::end_beats)
        .fold(f32::NEG_INFINITY, f32::max);
    if end - start > MAX_LENGTH_BEATS {
        bail!("the span is longer than {MAX_LENGTH_BEATS} beats");
    }
    Ok((start, end))
}

/// One MIDI block holding the notes each block at `indices` lets sound, cut
/// at their block's end and placed relative to the first start.
fn merged_midi(lane: &Track, indices: &[usize]) -> Result<Block> {
    let (start, end) = span(lane, indices)?;
    let mut notes: Vec<Note> = indices
        .iter()
        .map(|&index| &lane.blocks[index])
        .flat_map(|block| {
            block
                .notes
                .iter()
                .filter(|note| note.start_beats >= 0.0 && note.start_beats < block.length_beats)
                .map(move |note| Note {
                    start_beats: block.start_beats - start + note.start_beats,
                    length_beats: note.length_beats.min(block.length_beats - note.start_beats),
                    ..*note
                })
        })
        .collect();
    if notes.len() > MAX_NOTES_PER_BLOCK {
        bail!("more than {MAX_NOTES_PER_BLOCK} notes in one block");
    }
    notes.sort_by(|a, b| a.start_beats.total_cmp(&b.start_beats));
    let mut block = Block::new(
        consolidated_name(&lane.blocks[indices[0]].name),
        TrackKind::Midi,
        start,
        end - start,
        "",
    );
    block.notes = notes;
    Ok(block)
}

/// A block name with the consolidate suffix, never doubled.
fn consolidated_name(first: &str) -> String {
    format!("{}{SUFFIX}", first.trim_end_matches(SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "cassette-consolidate-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn app_in(dir: &TempDir) -> App {
        let mut app = App::new_silent().expect("silent app");
        app.project_path = Some(dir.0.join("song.cassette"));
        app
    }

    fn create(app: &mut App, track: usize, start_beats: f32, length_beats: f32) {
        app.submit(Event::CreateBlock {
            track,
            start_beats,
            length_beats,
        });
    }

    fn note(app: &mut App, track: usize, block: usize, start_beats: f32, length_beats: f32) {
        app.submit(Event::AddNote {
            track,
            block,
            pitch: 60,
            start_beats,
            length_beats,
            velocity: 100,
        });
    }

    fn consolidate(app: &mut App, blocks: &[(usize, usize)]) {
        app.submit(Event::ConsolidateBlocks {
            blocks: blocks
                .iter()
                .map(|&(track, block)| TrackBlock { track, block })
                .collect(),
        });
    }

    fn starts_and_lengths(block: &Block) -> Vec<(f32, f32)> {
        block
            .notes
            .iter()
            .map(|note| (note.start_beats, note.length_beats))
            .collect()
    }

    #[test]
    fn midi_blocks_merge_into_one_spanning_block_with_the_gap_kept() {
        let dir = TempDir::new();
        let mut app = app_in(&dir);
        create(&mut app, 0, 2.0, 4.0);
        create(&mut app, 0, 10.0, 2.0);
        note(&mut app, 0, 0, 1.0, 1.0);
        note(&mut app, 0, 1, 0.5, 1.0);
        consolidate(&mut app, &[(0, 0), (0, 1)]);

        let lane = &app.song.tracks[0].blocks;
        assert_eq!(lane.len(), 1, "{}", app.status);
        assert_eq!((lane[0].start_beats, lane[0].length_beats), (2.0, 10.0));
        assert_eq!(lane[0].name, "1 (cons.)");
        assert_eq!(starts_and_lengths(&lane[0]), vec![(1.0, 1.0), (8.5, 1.0)]);
        assert_eq!(app.cursor, Cursor { track: 0, block: 0 });
    }

    #[test]
    fn notes_a_block_trims_away_are_dropped_and_crossing_ones_cut() {
        let dir = TempDir::new();
        let mut app = app_in(&dir);
        create(&mut app, 0, 0.0, 4.0);
        note(&mut app, 0, 0, 0.0, 1.0);
        note(&mut app, 0, 0, 3.0, 4.0);
        app.song.tracks[0].blocks[0].notes.push(Note {
            pitch: 64,
            start_beats: 6.0,
            length_beats: 1.0,
            velocity: 90,
        });
        app.submit(Event::ResizeBlock {
            track: 0,
            block: 0,
            length_beats: 3.5,
        });
        create(&mut app, 0, 4.0, 4.0);
        consolidate(&mut app, &[(0, 0), (0, 1)]);

        let block = &app.song.tracks[0].blocks[0];
        assert_eq!(starts_and_lengths(block), vec![(0.0, 1.0), (3.0, 0.5)]);
        assert_eq!(block.length_beats, 8.0);
    }

    #[test]
    fn each_track_gets_its_own_block_and_unnamed_blocks_stay() {
        let dir = TempDir::new();
        let mut app = app_in(&dir);
        for track in [0, 1] {
            create(&mut app, track, 0.0, 2.0);
            create(&mut app, track, 4.0, 2.0);
            create(&mut app, track, 8.0, 2.0);
        }
        consolidate(&mut app, &[(1, 1), (0, 0), (1, 0), (0, 1), (0, 1), (5, 0)]);

        for track in [0, 1] {
            let lane = &app.song.tracks[track].blocks;
            assert_eq!(lane.len(), 2);
            assert_eq!((lane[0].start_beats, lane[0].length_beats), (0.0, 6.0));
            assert_eq!(lane[1].start_beats, 8.0);
        }
        assert!(app.status.contains("2 tracks"), "{}", app.status);
    }

    #[test]
    fn one_undo_restores_every_original_block() {
        let dir = TempDir::new();
        let mut app = app_in(&dir);
        create(&mut app, 0, 0.0, 2.0);
        create(&mut app, 0, 4.0, 2.0);
        create(&mut app, 1, 1.0, 2.0);
        let before = app.song.clone();
        consolidate(&mut app, &[(0, 0), (0, 1), (1, 0)]);
        assert_ne!(app.song, before);
        app.submit(Event::Undo);
        assert_eq!(app.song, before);
    }

    #[test]
    fn nothing_named_changes_nothing() {
        let dir = TempDir::new();
        let mut app = app_in(&dir);
        create(&mut app, 0, 0.0, 2.0);
        let before = app.song.clone();
        consolidate(&mut app, &[(0, 3)]);
        assert_eq!(app.song, before);
        assert!(app.status.contains("select blocks"), "{}", app.status);
    }

    #[test]
    fn audio_blocks_render_to_one_wav_as_long_as_the_span() {
        let dir = TempDir::new();
        let tone: Vec<f32> = (0..EXPORT_SAMPLE_RATE / 2)
            .map(|i| (i as f32 / 100.0).sin() * 0.5)
            .collect();
        let tone_path = dir.0.join("tone.wav");
        wav::encode_pcm16(&tone_path, &tone, 1, EXPORT_SAMPLE_RATE).expect("write tone");
        let mut app = app_in(&dir);
        for start_beats in [0.0, 3.0] {
            app.submit(Event::ImportSample {
                track: 2,
                path: tone_path.to_string_lossy().into_owned(),
                start_beats,
            });
        }
        let effects_before = app.song.tracks[2].effects.clone();
        let before = app.song.clone();
        consolidate(&mut app, &[(2, 0), (2, 1)]);

        let lane = &app.song.tracks[2].blocks;
        assert_eq!(lane.len(), 1, "{}", app.status);
        let block = &lane[0];
        assert_eq!((block.start_beats, block.length_beats), (0.0, 4.0));
        assert_eq!(block.source_bpm, 0.0);
        let expected = dir.0.join(CONSOLIDATED_DIR).join("Audio_1-1.wav");
        assert_eq!(Path::new(&block.source), expected);
        assert_eq!(app.song.tracks[2].effects, effects_before);

        let sample = wav::decode(&expected, usize::MAX).expect("decode consolidated");
        let frames_per_beat = EXPORT_SAMPLE_RATE as f32 * 60.0 / app.song.bpm();
        assert_eq!(sample.frames.len(), (4.0 * frames_per_beat) as usize);
        let loudest = |from: f32, to: f32| {
            sample.frames[(from * frames_per_beat) as usize..(to * frames_per_beat) as usize]
                .iter()
                .fold(0.0f32, |peak, v| peak.max(v.abs()))
        };
        assert!(loudest(0.1, 0.9) > 0.4, "the first clip at unity");
        assert!(loudest(1.1, 2.9) < 0.001, "the gap is silent");
        assert!(loudest(3.1, 3.9) > 0.4, "the second clip at unity");
        assert!(app.ui_state().export_progress.is_none());

        app.submit(Event::Undo);
        assert_eq!(app.song, before);
    }

    #[test]
    fn a_second_consolidate_takes_the_next_file_and_keeps_one_suffix() {
        let dir = TempDir::new();
        let tone_path = dir.0.join("tone.wav");
        wav::encode_pcm16(&tone_path, &[0.2; 4410], 1, EXPORT_SAMPLE_RATE).expect("write tone");
        let mut app = app_in(&dir);
        app.submit(Event::ImportSample {
            track: 2,
            path: tone_path.to_string_lossy().into_owned(),
            start_beats: 0.0,
        });
        consolidate(&mut app, &[(2, 0)]);
        consolidate(&mut app, &[(2, 0)]);
        let block = &app.song.tracks[2].blocks[0];
        assert!(block.source.ends_with("Audio_1-2.wav"), "{}", block.source);
        assert_eq!(block.name, "tone (cons.)");
    }
}
