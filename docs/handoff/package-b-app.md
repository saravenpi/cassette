# Package B: app logic (clipboard, tracks, batch notes, project path)

Rust + Tauri DAW. Read `docs/handoff/README.md` first for the shared rules.
First command: `git reset --hard <handoff commit>` and confirm with
`git log --oneline -1`.

Then read `ROADMAP.md` section "Phase 9" (all Decisions bullets),
`src/events/types.rs` (the new events: `AddNotes`, `EditNotes`, `DeleteNotes`,
`QuantizeBlock.notes`, `AddTrack`, `DeleteTrack`, `DuplicateTrack`,
`RenameTrack`, `CopyRange` / `CutRange` / `DeleteRange` / `PasteRange` /
`DuplicateRange` with `BeatRange`, `SetEffectSidechain`,
`SetInstrumentEnabled`, `SetInstrumentTone`), `src/ui/view.rs` (new snapshot
fields: `TrackView.instrument_enabled/cutoff/resonance/detune/sub`,
`EffectView.id/sidechain`, `UiState.project_path/project_dirty/clipboard`),
`src/models/effect.rs` (`Effect.id`, `Effect.sidechain`, `MAX_EFFECTS = 8`,
duplicates now allowed), `src/models/instrument.rs`, and all of `src/app/` (the
stubs are in `clipboard.rs`, `tracks.rs`, `notes.rs`, `effects.rs`; the undo
model is in `history.rs`).

## Files you may touch

`src/app/**`, `src/models/effect.rs`, `src/models/track.rs`,
`src/models/song.rs`, `src/models/note.rs`, and `src/ui/view.rs` (only the
`UiState` and `TrackView` fill helpers; do not change field shapes). Other agents
edit `src/core/**`, `src/models/instrument.rs` and `app/**` in parallel.

Engine facts you can rely on: `self.engine.set_effects(track, &chain) -> bool`
(on false, set `resync_pending`); `AudioEngine::resync(&Song)` rebuilds the audio
side; `RtCommand::SetTrackCount`; `RtCommand::SetTrackInstrument { track,
instrument }` carries the whole `Instrument`, including the new fields and
`enabled`.

## Deliverable

1. **Effects** (`effects.rs`).
   - Effect ids: `add_effect` gives the new effect an id unique within the song
     (a counter on `Song` with serde default, or max+1 per track). `sanitize`
     gives unique non-zero ids to effects loaded with id 0 or duplicate ids.
   - Allow the same kind more than once: remove the duplicate refusal and its
     test. Refuse only past `MAX_EFFECTS` (8).
   - `set_effect_sidechain(track, index, source)`: refuse a kind that does not
     take a sidechain (`kind.takes_sidechain()`), refuse a source equal to its
     own track or out of range; `None` clears it. Status lines such as
     "Compressor listens to Kick" and "Compressor listens to its own input".
     Undo kind: Edit.
   - Track deletion and insertion: deleting a track clears sidechains that
     pointed at it and shifts those pointing at later tracks down by one;
     duplicating inserts a track, so sidechains pointing at later tracks shift
     up. Write one helper for this remap.
2. **Batch notes** (`notes.rs`).
   - `add_notes` clamps each note into the block; refuses past
     `MAX_NOTES_PER_BLOCK` (all or nothing, with a status line); refuses audio
     blocks.
   - `edit_notes`: absolute values per index, each clamped like `move_note` /
     `resize_note`. Bad indices are ignored; the status says how many were
     edited.
   - `delete_notes`: dedupe, remove in descending index order.
   - `quantize_block` with a non-empty `notes` list quantizes only those.
   - History: `AddNotes`, `DeleteNotes`, `QuantizeBlock` are Edit. `EditNotes`
     is `Kind::Stream(Gesture::EditNotes(track, block))`, so a multi-note drag is
     one undo step.
3. **Tracks** (`tracks.rs`).
   - `add_track(kind)` appends a track named "MIDI N" or "Audio N" (N = next
     free number) with the kind's default preset. Refuse past `MAX_TRACKS`
     (16, in `crate::core`).
   - `delete_track(track)` refuses deleting the last track. It moves the cursor
     to a valid track, frees sample slots no longer referenced by any block (see
     how `samples.rs` tracks slots and `loaded_samples`), and remaps sidechains.
   - `duplicate_track(track)` inserts a copy right after: blocks, notes, effects
     with NEW ids, instrument and mix. Name it "<name> copy"; remap sidechains.
   - `rename_track(track, name)` trims, refuses empty, caps at 32 chars.
   - All four change the track count. Rebuild the engine side with
     `self.engine.resync(&self.song)` followed by the app's resync helper, as
     history restore does (`App::resync`). Resize `MeterBank`, `note_revisions`
     and any per-track arrays.
   - Undo: Edit, including the track count change. Verify the meter bank size
     after undoing an add or delete.
   - `set_instrument_enabled` and `set_instrument_tone(cutoff, resonance,
     detune, sub)`: clamp to the documented ranges in `instrument.rs`, send
     `RtCommand::SetTrackInstrument`, set a status line. Undo:
     `SetInstrumentEnabled` is Edit, `SetInstrumentTone` is
     `Stream(Gesture::InstrumentTone(track))`.
   - The `preset` name in `TrackView` (`preset_index`) must still match a preset
     when only `enabled` differs: compare with `enabled` forced to true.
4. **Arrangement clipboard** (`clipboard.rs`). The model is a `BeatRange` (track
   run plus beat span).
   - `copy_range(range)` normalizes the range (swap if reversed, clamp tracks,
     refuse an empty span) and stores on `App` a `Clipboard { length_beats,
     lanes: Vec<Vec<Block>> }`. Each lane holds the blocks, or the parts of
     blocks, inside the span, with starts relative to the range start. A block
     crossing an edge is cut there: reuse `Block::split_off` (see `split_block`
     in `timeline.rs`; it handles audio `offset_seconds` and MIDI notes).
   - `delete_range` removes the material inside the range from each track in the
     run, splitting blocks that cross the edges and keeping the outside parts.
     It does not shift time.
   - `cut_range` is copy then delete.
   - `paste_range(track, start_beats)` places lane i on track `track + i`. Lanes
     past the last track are dropped with a status note. It clears the target
     span first (Ableton overwrites), then inserts the blocks. A sample block
     keeps its source and slot.
   - `duplicate_range(range)` pastes a copy at `range.end_beats` on the same
     tracks and leaves the clipboard untouched.
   - Every op pushes the mirrors and notes of every touched track, respects
     `MAX_BLOCKS`, and is one Edit undo step. Fill `UiState.clipboard` with
     `ClipboardView { tracks, length_beats }`. The status line says what
     happened ("copied 2 tracks, 4 beats").
5. **Project path, dirty flag and `.cassette`** (`export.rs`, `mod.rs`).
   - `App.project_path: Option<PathBuf>`. `export_project(Some(path))` writes
     there and remembers it. `export_project(None)` writes to the remembered
     path if any, else the default path. Loading remembers the path.
   - The default extension becomes `.cassette` (JSON inside). A given path with
     no extension gets `.cassette` appended. `latest_project_path` and loading
     accept both `.cassette` and `.json`.
   - Dirty: keep an edit revision that history bumps whenever it records or
     merges a step and whenever undo or redo runs. Saving or loading stores the
     saved revision; `project_dirty = revision != saved`. A fresh app is clean.
   - Fill `UiState.project_path` (display string) and `project_dirty`.
   - Status lines: "saved <name>.cassette", "loaded <name>.cassette".
6. **Tests** in your files. Each clipboard op, including blocks split at both
   edges for MIDI and audio, multi-track paste, paste past the last track,
   duplicate, one undo step each. Track add, delete, duplicate, rename, with
   refusals, undo and the sidechain remap. Batch notes, and `EditNotes`
   coalescing into one undo step. Quantize with a subset. Effect ids unique
   (including sanitize), duplicates allowed, sidechain refusals. Save remembering
   the path, `.cassette` appended, `.json` still loading, and the dirty flag
   (clean, edit, dirty, save, clean; undo after save is dirty again). Instrument
   bypass and tone clamps.

## Gates and commits

`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Do not
weaken existing tests except the duplicate-kind refusal test, which the new rule
replaces. Commit with Conventional Commits, for example
`feat(app): copy, cut, paste and duplicate arrangement ranges`,
`feat(app): add, delete, duplicate and rename tracks`,
`feat(app): edit notes in batches`,
`feat(app): remember the project path and save as .cassette`.

Final report, under 300 words: behaviour decisions (status strings, naming,
clipboard semantics), anything you relied on from the engine and could not
verify, the test count, the commit hashes and branch.
