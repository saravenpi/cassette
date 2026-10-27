# Package D: arrangement UI (selection, tracks, save flow, instruments, sidechain)

Rust engine with a SvelteKit 5 + Tauri app in `app/`. Read
`docs/handoff/README.md` first for the shared rules. First command:
`git reset --hard <handoff commit>` and confirm with `git log --oneline -1`.

Read `ROADMAP.md` section "Phase 9" (every Decision bullet) and the Ableton
arrangement notes it cites
(https://www.ableton.com/en/live-manual/12/arrangement-view/). Then read
`app/src/lib/wire.ts` (the contract: `BeatRange`, `ClipboardView`, the range
events `copyRange` / `cutRange` / `deleteRange` / `pasteRange` /
`duplicateRange`, track events `addTrack` / `deleteTrack` / `duplicateTrack` /
`renameTrack`, `setEffectSidechain`, `setInstrumentEnabled`,
`setInstrumentTone`, and the snapshot fields `projectPath`, `projectDirty`,
`clipboard`, `TrackView.instrumentEnabled/cutoff/resonance/detune/sub`,
`EffectView.id/sidechain`, `EffectSpecView.takesSidechain`), then
`app/src/lib/session.svelte.ts`, `keys.ts`, `keyboard.ts`, `menu.ts`,
`menu.svelte.ts`, `components/Timeline.svelte`, `TrackLane.svelte`,
`TrackHeader.svelte`, `Block.svelte`, `Ruler.svelte`, `Transport.svelte`,
`Sidebar.svelte`, `VstPane.svelte`, `EffectsPane.svelte`, `Device.svelte`,
`DeviceChain.svelte`, `Knob.svelte`, `DetailPane.svelte`, `api.ts`,
`app/src-tauri/src/lib.rs`, `app/src-tauri/capabilities/default.json` and
`app/src/app.css`.

## Files you may touch

Everything under `app/src/**` and `app/src-tauri/**` EXCEPT
`app/src/lib/components/PianoRoll.svelte`, `app/src/lib/pianoRoll.ts`,
`app/src/lib/pianoRoll.test.ts`, `app/src/lib/noteSelection.ts` and
`app/src/lib/noteSelection.test.ts`, which package C owns. No Rust files outside
`app/src-tauri/`. The engine and app logic land in other worktrees in parallel;
on your branch the new events are accepted but do nothing and the new snapshot
fields read as defaults. Build against the contract in `wire.ts` and test
against stubbed snapshots.

## Deliverable

1. **Time selection** in the track view (Ableton arrangement model).
   - A new store `app/src/lib/selection.svelte.ts` with pure helpers in
     `selection.ts` and tests. State: `range: BeatRange | null` (firstTrack,
     lastTrack, startBeats, endBeats), normalized.
   - Pointer drag on empty lane space draws a selection rectangle over a run of
     tracks (snapped to `gridStep(zoom)`, Alt frees it). A plain click on empty
     lane space (no drag) clears the selection and puts the marker there, as
     double-click does today. Shift+click extends the selection to the clicked
     beat and track. Clicking a block selects that block's span on its track
     (and keeps the existing block selection). `Esc` clears.
   - Draw the selection as a translucent `--green` band over the lanes with a
     1 px border and edge handles you can drag to adjust. Show its length in
     bars.beats in the header corner readout.
   - Resizing the window, zooming and scrolling must keep the band aligned
     (compute from beats, never from stored pixels).
2. **Selection commands.** Keyboard, handled in `keys.ts` / `keyboard.ts`:
   `Ctrl/Cmd+C` copy, `Ctrl/Cmd+X` cut, `Ctrl/Cmd+V` paste at the marker on the
   selected track (marker = the transport's marker beat), `Ctrl/Cmd+D` duplicate
   the selection (when a range exists; with only a block selected it still
   duplicates the block, as today), `Delete` / `Backspace` delete the range when
   one exists (else the block, as today), `Ctrl/Cmd+L` loop the selection
   (`setLoopRegion` with the range's beats and turns looping on; with no
   selection it clears the loop region). Make sure these never fire while the
   piano roll grid or a text field has focus. Mirror them in a context menu on
   the selection band (Cut, Copy, Paste, Duplicate, Delete, Loop selection,
   Clear selection). Paste is disabled when `snapshot.clipboard === null`.
   Update `keys.test.ts` and `menu.test.ts`.
3. **Loop selection visuals.** The loop region already draws on the ruler; add a
   "Loop selection" button to the selection menu and show it clearly. `Ctrl/Cmd+L`
   twice toggles it off.
4. **Track management.** In the track header: a `+` control at the bottom of the
   header column with "Add MIDI track" and "Add audio track" (`addTrack`), and a
   track context menu with Rename (inline, or double-click on the name,
   `renameTrack`; Enter commits, Esc cancels, and the global keymap must not
   swallow keys while editing), Duplicate, Delete (`duplicateTrack`,
   `deleteTrack`). Keyboard: `Ctrl/Cmd+T` adds an audio track, `Ctrl/Cmd+Shift+T`
   adds a MIDI track. Disable Delete when only one track is left and the add
   controls at 16 tracks. Row heights are stored per track index in the timeline;
   prune them when tracks go (use `pruneHeights`).
5. **Save, Save As, Open, Export.**
   - Tauri: add `dialog:allow-save` to the capabilities file. In `api.ts` add
     `pickSaveProject(defaultName)` using `save()` from
     `@tauri-apps/plugin-dialog` with a `.cassette` filter and name, and
     `pickExportWav(defaultName)` with a `.wav` filter. `pickProject()` (open)
     accepts `.cassette` and `.json`.
   - `session.saveProject()`: if `snapshot.projectPath` is null, open the save
     dialog, then send `exportProject { path }`. Otherwise send
     `exportProject { path: null }`, which saves to the remembered path.
     `session.saveProjectAs()` always opens the dialog. Cancel sends nothing.
     `session.exportAudio()` opens the WAV dialog and sends `exportAudio { path }`.
   - Keyboard: `Ctrl/Cmd+S` save, `Ctrl/Cmd+Shift+S` save as, `Ctrl/Cmd+O` open.
     The transport's Save, Load and Export buttons use the same methods
     (rename Load to Open, add Save As to a small menu next to Save).
   - Show the project name (file stem of `projectPath`, or "Untitled") in the
     transport with a dot when `projectDirty`. Set `document.title` to match.
   - Drop handling: `.cassette` and `.json` dropped on the window load the
     project (extend the check in `+page.svelte`).
6. **Instruments tab** (rename the "VST" tab to "Instruments" and rework
   `VstPane.svelte`, renaming the file to `InstrumentsPane.svelte`). List the
   presets grouped by role (Synth, Keys, Bass, Pad, Drums), one row each, a
   double-click or Enter sets the preset on the selected track (`setPreset`), and
   the current one is marked. The preset group table comes from the engine's
   `instrument.rs`; until it is in the snapshot, hold the grouping in a small
   `app/src/lib/presets.ts` map keyed by preset name (Lead, Bass, Pad, Pluck,
   Kick, Snare, Hat, Supersaw, Sub Bass, Acid, Keys, Bell, Organ, Strings,
   Brass, 808, Tom, Clap, Rim) with a test that every name in a stubbed snapshot
   has a group. A preset not in the map goes under "Other". Remove the text that
   says the engine hosts no plugin format and replace it with one honest line:
   "Built-in instruments. Plugin hosting is not implemented."
7. **Instrument card**: the activator toggle (`setInstrumentEnabled`) on the
   Instrument device, a bypassed instrument dims its body, and a "Tone" row of
   four dials (Cutoff, Resonance, Detune, Sub) using `Knob.svelte`, sent through
   `setInstrumentTone` with the per-frame keyed queue the effect dials use and a
   `checkpoint` at drag start.
8. **Sidechain picker** on a device whose spec has `takesSidechain`: a small
   "Sidechain" selector at the bottom of the card, a segmented or select control
   listing "Off" (own input) and every other track by name, sending
   `setEffectSidechain { track, index, source }`. Show the source track's
   colour swatch. Use the effect's `id`, not its index, as the keyed-each key in
   `DeviceChain.svelte` so cards keep their state when the chain re-sorts.
9. **Effect browser** lists all 12 kinds with the new blurbs (already in
   `chain.ts`), and a kind can now be added more than once: remove the logic
   that hides kinds already in the chain (`chain.ts` helpers and tests), and
   disable adding at 8 effects with a tooltip.
10. **Review leftovers** worth fixing while you are in these files: the first
    sidebar tab labels, the focus ring on every new control, `aria-label` on the
    selection band and the track controls, and keeping the status bar hints in
    step with the new shortcuts.
11. **Tests** (`bun test`): selection normalization and hit tests, the range
    commands' event payloads, the save logic with a stubbed dialog (new project
    opens the dialog, saved project does not, cancel sends nothing), the project
    name and dirty label, the keys (`Ctrl+S`, `Ctrl+Shift+S`, `Ctrl+O`,
    `Ctrl+T`, `Ctrl+Shift+T`, `Ctrl+L`, `Ctrl+C/X/V/D` with and without a
    selection), the preset grouping, and the chain helpers.

## Design rules

Existing tokens only (`app.css`). Square corners, never bold, uppercase small
labels with letter-spacing like the existing `.eyebrow`, dividers instead of
boxes. Every control keyboard reachable and labelled, with a visible focus ring.
Style: tabs, single quotes, width 100, no trailing commas, Svelte 5 runes. A doc
block at the top of each new component; JSDoc above exports.

## Gates and commits

From `app/`: `bun run check` (0 errors, 0 warnings) and `bun test`. From the
repo root, the Tauri shell must still build clean: `cd app/src-tauri && cargo fmt
--check && cargo clippy --locked -- -D warnings`. Commit with Conventional
Commits, for example `feat(app): select time across tracks and cut, copy,
paste or loop it`, `feat(app): add, delete, duplicate and rename tracks`,
`feat(app): ask where to save and save as .cassette`,
`feat(app): browse instruments by role`, `feat(app): pick a sidechain source`.

Final report, under 300 words: components and stores added, interaction
decisions, anything in the contract you found missing, the test count, the
commit hashes and branch.
