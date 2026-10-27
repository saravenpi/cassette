# Package C: piano roll (selection, group edit, start-edge resize)

Rust engine with a SvelteKit 5 + Tauri app in `app/`. Read
`docs/handoff/README.md` first for the shared rules. First command:
`git reset --hard <handoff commit>` and confirm with `git log --oneline -1`.

Then read `ROADMAP.md` section "Phase 9" (the piano roll Decisions bullet is
your spec; it follows Ableton Live 12's MIDI note editor,
https://www.ableton.com/en/live-manual/12/editing-midi/). Then read
`app/src/lib/wire.ts` (new events `addNotes`, `editNotes` with `NoteEdit`,
`deleteNotes`, and `quantizeBlock` with a `notes` list),
`app/src/lib/components/PianoRoll.svelte`, `app/src/lib/pianoRoll.ts` and its
tests, `app/src/lib/session.svelte.ts` (how the roll is opened,
`request` / `dispatch`, the per-frame note queues `queueNoteMove` /
`queueNoteResize` / `queueNoteVelocity`, `quantizeSelected`),
`app/src/lib/queue.ts`, `app/src/lib/keyboard.ts`, `app/src/lib/keys.ts` and
`app/src/app.css` (the Kono tokens).

## Files you may touch

`app/src/lib/components/PianoRoll.svelte`, `app/src/lib/pianoRoll.ts`,
`app/src/lib/pianoRoll.test.ts`, and the new `app/src/lib/noteSelection.ts` plus
`app/src/lib/noteSelection.test.ts` (pure selection and edit math). Another
agent owns every other frontend file, including `session.svelte.ts`, `keys.ts`
and `keyboard.ts`, and edits them in parallel. Do not edit them. Use the
session's existing public methods (`request`, `dispatch`, `sendNow`, `report`,
`roll`, `snapshot`, `closeRoll`). For per-frame coalescing of `editNotes`
during a drag, write a small local coalescer in your own files: at most one
`editNotes` per animation frame per drag, latest values winning. Send a
`checkpoint` at drag start so each drag is one undo step; the engine merges a
stream of `editNotes` on one block into one step.

## Deliverable

1. **Selection model** (`noteSelection.ts`, pure, tested): a `Set<number>` of
   note indices.
   - Marquee: notes intersecting a beat and pitch rectangle. Shift+marquee adds
     to the selection.
   - Click selects one note; Shift+click toggles it; `Esc` clears;
     `Ctrl/Cmd+A` selects all.
   - Re-indexing after the engine deletes notes (`deleteNotes` removes
     indices): clear or remap the selection.
   - After `addNotes` (paste or duplicate), select the new notes. They are
     appended in order, so their indices are `oldLength .. oldLength + n`.
2. **Pointer interactions** (`PianoRoll.svelte`).
   - Pointer down on empty grid starts a marquee: a translucent rectangle in
     `--green` at low alpha with a 1 px border. A click without drag clears the
     selection. Double-click on empty grid adds a note at the snapped cell
     (`addNote`, as today) and selects it.
   - **Draw mode**: a pencil toggle button in the roll bar, plus the `B` key
     while the grid has focus (handle it in the grid's own keydown and stop
     propagation so the timeline's `B` split does not fire). It restores
     click-to-add; click-drag draws a note's length. In draw mode, clicking an
     existing note deletes it, as in Live.
   - Pointer down on a note: if it is not selected, select only it (Shift
     toggles). Then drag: moving drags EVERY selected note by the same beat and
     pitch delta, snapped to the grid (Alt frees it), clamped so no note leaves
     the block or the pitch range. Clamp the delta for the group, not per note,
     so the shape is kept.
   - **Edges**: extend `hitNote` in `pianoRoll.ts` to report
     `edge: 'start' | 'end' | null`. Dragging the LEFT edge changes start and
     length together, keeping the end fixed. Dragging the right edge changes the
     length. Either applies the same delta to every selected note, never shorter
     than the minimum length (0.0625 beats, `Note::MIN_LENGTH_BEATS`). The cursor
     shows `ew-resize` on both edges and `grab` / `grabbing` on the body. For
     tiny notes the two handles must not overlap.
   - Draw the dragged values locally until the snapshot catches up, as today's
     single-note drag does, but for N notes.
   - Wheel over a note changes velocity for the whole selection if that note is
     selected, else for that note (`editNotes`).
3. **Keyboard**, on the grid's keydown. Stop propagation only for the keys you
   handle; let `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+Y`, Space and `Ctrl+U` through to
   the window keymap.
   - Delete / Backspace deletes the selection (`deleteNotes`).
   - Left / Right nudges by the grid step (Alt for 1/64). Up / Down transposes
     by a semitone, Shift+Up / Down by an octave. Shift+Left / Right shortens or
     lengthens by the grid step.
   - `Ctrl/Cmd+A` selects all. Esc clears; Esc with an empty selection closes
     the roll, as today.
   - `Ctrl/Cmd+C` copies the selected notes to a module-level clipboard in your
     file (relative to the earliest start). `Ctrl/Cmd+X` cuts. `Ctrl/Cmd+V`
     pastes at the last empty-grid click position if there is one, else right
     after the copied notes' end, clamped into the block, as one `addNotes`.
     `Ctrl/Cmd+D` duplicates the selection right after its last end, as one
     `addNotes`.
   - Arrow edits send one `editNotes` per key press.
4. **Quantize**: the Quantize button quantizes the selection if one exists
   (`quantizeBlock` with `notes`), else all notes. `Ctrl+U` keeps working through
   the session's `quantizeSelected()`. Add an "N selected" readout to the bar
   that replaces the single-note info when more than one note is selected.
5. **Accessibility**: the grid's aria-label lists the main gestures, the
   selection count is announced through the existing aria-live info, and focus
   stays on the grid after edits.
6. **Tests** (`bun test`), for the pure parts: marquee hit test plus Shift add and
   toggle; group move clamping that keeps the shape at the block edges and pitch
   range; left-edge resize that keeps the end and clamps to the minimum length;
   right-edge resize across the group; paste and duplicate placement; `hitNote`
   start and end edges including tiny notes; selection re-indexing after a
   delete; arrow nudge and transpose math.

## Design rules

Use only existing tokens from `app.css` (`--green`, `--on-green`, `--border`,
`--border-subtle`, `--text`, `--text-subtle`, `--dim`, `--surface`, `--alert`).
Square corners, never bold (the Undefined font is never bold). Selected notes get
a brighter fill or outline than unselected ones; velocity opacity stays. Style:
tabs, single quotes, width 100, no trailing commas. Svelte 5 runes. JSDoc above
exported functions; update the component's doc block.

## Gates and commits

From `app/`: `bun run check` (0 errors, 0 warnings) and `bun test`. Commit with
Conventional Commits, for example
`feat(app): select, move and resize several notes in the piano roll`.

Final report, under 250 words: interaction decisions, keys handled, what you
could not do without touching `session.svelte.ts` (if anything), the test count,
the commit hash and branch.
