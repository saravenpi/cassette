# cassette: roadmap

A session DAW inspired by Ableton Live. The audio engine is Rust and owns all
timing and synthesis. The interface is a SvelteKit frontend in `app/` behind a
Tauri shell; keys and clicks both drive it.

---

## Status

| Phase | Scope | Status |
|-------|-------|--------|
| Phase 1 | Critical fixes | Done |
| Phase 2 | Core audio engine | Done |
| Phase 3 | Interface and event plumbing | Done |
| Phase 4 | Polish and features | Done |
| Phase 5 | Session view and blocks | Superseded by Phase 6 (2026-09-24) |
| Phase 6 | Timeline and the sample path | Done (v0.7.0, 2026-09-26) |
| Phase 7 | Note data in MIDI blocks | Next |

The engine crate builds with no webview and no audio headers, plays the
polyphonic synth from the keyboard, and starts a sample when the playhead
reaches its block. The project is called cassette; it was zboub69 until v0.7.0.

Build state, measured on 2026-10-26 at v0.7.0: `cargo test` is green end to end
(154 library unit tests, 11 engine integration, 10 MIDI/OSC adapter, 3
red-zone, 7 wire), `cargo clippy --all-targets -- -D warnings` and
`cargo fmt --check` are clean, and `cd app && bun run check` and
`cd app && bun test` (83 tests) are clean.

The desktop app has been run on real audio hardware: samples play from the
timeline and the synth plays from the keyboard (verified by ear, 2026-10-26).

The step sequencer, the step grid and the pattern model were removed in Phase 5:
the session view replaced them. Phase 6 replaced the session view with a
timeline. Sections below marked superseded describe what went away.

The step sequencer, the step grid and the pattern model were removed in Phase 5:
the session view replaced them. Phase 6 replaces the session view with a
timeline. Sections below marked superseded describe what went away.

---

## Phase 1: Critical fixes (done)

- `src/events/bus.rs`: fan-out, bounded crossbeam channels; slow subscribers are
  skipped instead of stalling producers.
- `src/core/audio.rs`, `src/core/render.rs`: red-zone contract, no allocation,
  no locking, no I/O, no panics in the render path.
- `tests/test_rt_no_alloc.rs`: `assert_no_alloc` global allocator, debug-only,
  driving the render loop and the command path.
- `Cargo.toml`: `crossbeam`, `rtrb`, `assert_no_alloc`; optional `cpal`.

## Phase 2: Core audio engine (done)

### 2.1 Sample-accurate transport

Position is a frame counter advanced in [`core::render`]'s `Transport`,
published to the interface through `SharedState` atomics. Tempo changes re-scale
the frame phase so the musical position never drifts.

[SUPERSEDED by: Phase 5 session view, 2026-09-24] Loop points were implicit
(loop the whole pattern). With the step grid gone, looping is a flag the
transport has no material to act on yet.

### 2.2 Audio callback and backend trait

`core::audio::AudioBackend` has four implementations: `CpalBackend`
(feature `cpal`), `SubprocessBackend` (ffplay pipe), `TcpStreamBackend`
(loopback PCM for SSH) and `NullBackend`. The render closure owns the synth and
the transport, so nothing is shared behind a lock.

### 2.3 Polyphonic synth and mixer

`core::synth` holds fixed arrays (`MAX_TRACKS × MAX_VOICES`) with per-track
volume, pan, mute and solo. Voices use a 5-waveform oscillator and an ADSR
envelope with precomputed per-sample increments.

### 2.4 Plugin hosting (optional, later)

Not started. The natural seam is a `Processor` trait behind `RtCommand`, with
`clack-host` or `nih-plug` implementing it.

## Phase 3: Interface and event plumbing (done)

### 3.1 Session view

[SUPERSEDED by: Phase 5 session view, 2026-09-24] The Mixer (track list, meters,
mute/solo), Pattern (step grid with a beat ruler) and Instrument (waveform,
envelope, step pitch) panes were terminal widgets. They went with the terminal
interface. The webview draws a transport header, a track list with blocks and a
status footer instead; see 5.1. [SUPERSEDED by: Phase 6 timeline, 2026-09-24] The
webview now draws a transport bar, a left sidebar and the timeline; see 6.1 and
6.5.

### 3.2 Event-driven architecture

The webview sends an `Event` as JSON through the Tauri `send` command, and the
MIDI and OSC adapters in `input/` produce the same values. Both funnel through
`App::submit`, so there is exactly one path into the app. The engine thread
drains them once per frame, applies them, then publishes a fresh `UiState`
snapshot the webview polls. The interface holds no state owned by `core`.

### 3.3 Keymap

[SUPERSEDED by: Phase 5 session view, 2026-09-24] `input::handler` and
`input::keymap` read raw terminal key codes; there is no terminal to read them
from now.

The keymap lives in `app/src/lib/keys.ts`, in the webview. It maps a
`KeyboardEvent` to an `Event` or to a MIDI pitch:

| Key | Action |
|-----|--------|
| `space` | play / stop |
| `L` | toggle looping |
| `up` `down` | move between tracks |
| `left` `right` | move between blocks on the selected track |
| `N` / `enter` | add a block to the selected track |
| `backspace` / `del` | delete the block under the cursor |
| `C` | cycle the track kind, MIDI or audio |
| `R` | rename the block under the cursor |
| `esc` | cancel the rename |
| `M` `O` | mute / solo the selected track |
| `-` `=` | track volume |
| `[` `]` | pan left / right |
| `,` `.` | tempo |
| `Z` `X` | shift the play octave |
| `A W S E D F T G Y H U J K` | play notes, 13 semitones from C |

Commands sit on keys the piano row does not use. Notes are checked first, so a
key bound to both would make the command silently unreachable, which is why
delete is `backspace` rather than `D` and why solo is `O` rather than `S`.
`keys.ts` throws at load if a clash is ever introduced. `app/src/lib/keys.test.ts`
covers the keymap; run it with `cd app && bun test`.

## Phase 4: Polish and features (done)

### 4.1 Offline rendering / bouncing

[SUPERSEDED by: Phase 5 session view, 2026-09-24] `core::offline` and the `E`
bounce were removed with the step grid: with no note data to schedule, a bounce
would have written silence.

### 4.2 Level metering

`core::meter` provides smoothed `LevelMeter`s and a `MeterBank` for the track
list. The `PerformanceRecorder` was removed with the pattern it wrote into;
capturing live notes needs block content, which does not exist yet.

### 4.3 Additional input devices

`input::MidiInputAdapter` and `input::OscInputAdapter` translate MIDI and OSC
into the shared event vocabulary. Hardware transport is not wired up yet; the
adapters and their tests are in place.

### 4.4 Alternate renderers

[SUPERSEDED by: Phase 5 session view, 2026-09-24] `ui::view::View` rendered
`UiState` to plain text. There is no plain-text renderer and no CLI binary: the
engine crate is a library, and `--headless` and `--stream-port` went with it.
`ui/view.rs` survives as the serializable `UiState` snapshot alone.
[SUPERSEDED by: the doc comment was corrected, 2026-09-24] This entry used to
note that the `TcpStreamBackend` doc comment in `core/audio.rs` still said no
front end starts the backend, contradicting the `ZBOUB_STREAM_PORT` route in 5.5.
The comment now names that route.

---

## Phase 5: Session view and blocks (superseded by Phase 6, 2026-09-24)

### 5.1 Track list replaces the step grid

[SUPERSEDED by: Phase 6 timeline, 2026-09-24] The main view is a list of tracks.
Each row carries its name, kind (MIDI or audio), preset or waveform, its blocks,
a `+` slot, a level meter, its volume and pan, and mute and solo buttons. The step
sequencer, the step grid, the pattern model, the step clock and the performance
recorder are gone. The timeline in 6.1 and 6.2 keeps the tracks and drops the
view's `+` slot: a block now has a position and a length instead of an append
order.

### 5.2 Blocks

[SUPERSEDED by: Phase 6 timeline, 2026-09-24] A block is a named slot holding no
content yet. `N` or `enter` appends one, `backspace` or `del` deletes the block
under the cursor (and asks for a full resync, because the audio-side mirror only
appends), `R` renames the block under the cursor, and left/right move between
blocks. A block now carries a start, a length and a source and is addressed by
position; see 6.2.

### 5.3 Transport

`Transport` survives the sequencer: it advances a frame counter and reports the
position in beats, published through `SharedState`. [SUPERSEDED by: Phase 6
timeline, 2026-09-24] It schedules nothing, so a project with no sounding notes
is silent until you play the keyboard. It now starts the blocks the playhead
crosses, so a project with placed audio blocks is not silent; see 6.1.

### 5.4 Still to do

[SUPERSEDED by: Phase 6 timeline, 2026-09-24] Engine side. Reading and writing a
song file, block content, block editing UI, scenes and follow actions. The
current list is in 6.6.

Interface side. The desktop app has never been run against real audio hardware,
so treat "it makes noise" as unverified until someone listens on a machine with a
working output device.

### 5.5 Audio-over-SSH front door (done, 2026-09-24)

The desktop app now starts the streaming backend when `CASSETTE_STREAM_PORT` (the legacy `ZBOUB_STREAM_PORT` still works) is set
to a `u16` greater than zero. Start it from `app/`:

```bash
CASSETTE_STREAM_PORT=9999 bun run tauri dev
```

Unset, empty, unparseable or zero keeps the normal device backend, so the default
app does not stream. A failed start falls back to the normal backend, then to the
silent backend, so the window always opens. The app header now renders
`backendNote`, which carries the stream's port. See
[music-over-ssh.md](./docs/music-over-ssh.md).

### 5.6 Keymap tests (done, 2026-09-24)

`app/src/lib/keys.test.ts` unit-tests the keymap in `app/src/lib/keys.ts`. A
`test` script in `app/package.json` runs them, so both work from `app/`:

```bash
bun test
bun run test
```

---

## Phase 6: Timeline and the sample path (done)

Phase 6 replaces the session view with a timeline: a left sidebar for samples,
plugins and effects, and a main area with a beat ruler, one lane per track, and
blocks placed at a beat position with a length in beats. Tracks stay MIDI or
audio; a sample dropped on a lane becomes an audio block, and a MIDI block is
created on a MIDI lane.

Each subsection below describes the interface as the code declares it, and says
where it stops. The pieces are `src/models/track.rs` (the block),
`src/core/render.rs` (the mirror and the playhead), `src/core/engine.rs` (the two
rings), `src/core/sample.rs` and `src/core/wav.rs` (the sample path),
`src/core/synth.rs` (the sample voice), `src/app/` (the edits, split into
`timeline`, `tracks`, `samples` and `transport`), `src/events/types.rs`,
`src/ui/view.rs`, the Tauri shell and the webview.

### 6.1 The beat timeline and lanes

The song is one timeline measured in beats from the top. `UiState` carries the
transport position (`transport_position_beats`) and the extent of the ruler
(`length_beats`, `bars`, `beats_per_bar`), and one `TrackView` per lane with its
`blocks` in start order. `TrackView::end_beats` reports where a lane's material
ends. `ruler_length` in `src/app/mod.rs` sets the ruler to the last block end
rounded up to a bar, never less than 64 beats, which is 16 bars at four beats
each.

Playback starts blocks, not notes. `RenderCore::process` records the transport
position before it renders a buffer and reads it again after; when the transport
is playing, `start_blocks` starts every block whose start lies in that span. A
crossing is noticed at the end of the buffer, so a block starts up to one buffer
late, and the sample is then entered at the frame offset the timeline asks for
(`(to_beats - block.start_beats) * frames_per_beat`), so its content stays in step
with the playhead. The crossing test handles a loop wrap.

What does not work:

- Seeking works since v0.4.0: a click or drag on the ruler sends `Event::Seek`,
  which becomes `RtCommand::Seek`. `Stop` still rewinds to frame 0. There is no
  loop range and no punch.
- `looping` is a flag with nothing to act on. No loop points exist, so the
  `L` key and the Loop button only record a preference.
- A block starts up to one buffer late, as above.

### 6.2 Blocks: position and length

A block is a rectangle on a lane: `Block { name, kind, start_beats,
length_beats, source }` in `src/models/track.rs`. Its geometry is real:

- `MIN_LENGTH_BEATS` is 0.25, `DEFAULT_LENGTH_BEATS` is 4.0, `MAX_LENGTH_BEATS`
  is 512.0.
- `Block::new` clamps the start to 0 or later and the length to at least
  `MIN_LENGTH_BEATS`; `set_start` and `set_length` keep those bounds.
- `contains(beats)` is half-open: the start belongs to the block, the end does
  not. `end_beats` is `start_beats + length_beats`.
- `Track::blocks` is a `Vec<Block>` kept in start order. `insert_block` finds the
  slot with `partition_point` and returns the index, `add_block(start_beats,
  length_beats)` creates a block of the track's kind and names it with the
  block count, `block_at(beats)` returns the first block containing a position,
  and `free_start()` is the ceiling of the last block's end, or 0 on an empty
  lane, which is where a new block lands.
- Nothing prevents two blocks from overlapping. `block_at` returns the earliest
  one in start order, and the audio side starts both when the playhead crosses
  them.
- A track holds at most `MAX_BLOCKS`, which is 64.

`src/app/timeline.rs` owns the edits, and each one pushes the lane it changed:

| Intent | What it does |
|--------|--------------|
| `CreateBlock` | Creates a block at the position and length the interface asked for, clamps both, and selects it |
| `AddBlock` | Appends one at `free_start()` with `DEFAULT_LENGTH_BEATS` |
| `PlaceBlock` | Moves a block to a new start, possibly onto another track, and re-inserts it in start order |
| `ResizeBlock` | Sets the length, clamped to the block limits |
| `DeleteBlockAt`, `DeleteBlock` | Remove the named block, or the one under the cursor |

The audio side never sees a `Block`. `App::mirror` flattens one lane into
`MirroredBlock { start_beats, end_beats, slot, gain }`, and `push_mirror` sends
that vector as `RtCommand::SetBlocks`, which replaces the track's mirror outright
rather than appending to it. A MIDI block mirrors `NO_SLOT` and a gain of 0; an
audio block mirrors the slot its source file occupies and the gain from
`sample_gain`, which is the instrument's own gain. The mirror carries no name:
`BlockName` and `RtCommand::AddBlock` are gone with the named slots.

The Phase 5 events `MoveBlock`, `AddBlock`, `DeleteBlock` and the rename events
are still in the vocabulary, so the keyboard cursor still works alongside the
pointer edits.

### 6.3 MIDI and audio tracks

`TrackKind` is `Midi` or `Audio`, and it carries the presentation and defaults
the interface needs: `name()` returns `MIDI` or `AUDIO`, `flipped()` returns the
other kind for the `C` key, and `default_preset()` returns `Lead` for a MIDI
track and `Pluck` for an audio track. A new `Song` is two MIDI tracks and two
audio tracks.

A block carries its own `kind`, so either kind can sit on either lane. The audio
side tells them apart by slot: `MirroredBlock.slot == NO_SLOT` means the block
holds no sample, and `start_blocks` skips it. `Track::is_audible(any_solo)`
applies the mute and solo rules.

`CycleTrackKind` flips a track between the two, taking the kind's default preset
with it and keeping the blocks. Dropping a sample flips a MIDI track to audio,
because a sample is what makes a lane an audio lane, and the status line says so.

A MIDI block plays nothing. Nothing holds notes: a block has a name, a kind, a
position, a length and a source, and `source` is empty for a MIDI block, so a
MIDI lane is a rectangle and no more. The synth is still played live from the
keyboard through `RtCommand::NoteOn`. Note data in a block is the biggest gap
left; see 6.6.

### 6.4 The sample path: decode, pool, ring, scheduled playback

The path a sample takes, control thread to mixer:

```
WAV file
  │ core::wav::decode               control thread: reads the disk, allocates
  ▼
SampleMsg { Begin, Frame, End }
  │ app::samples::push_load         into an rtrb ring, SAMPLE_RING_CAPACITY
  ▼
SamplePool                          audio thread: copies each message into memory
  │                                 allocated before the stream started
  ▼
SynthEngine::play_sample            one sample voice per track, started when the
  │                                 playhead reaches the block
  ▼
track mixer (volume, pan, mute, solo) ──► master
```

What works, and where:

- Decoding, in `src/core/wav.rs`. `decode(path, max_frames)` and `decode_bytes`
  walk the RIFF chunks by hand and read PCM at 8, 16, 24 and 32 bits plus IEEE
  float at 32 and 64, including the `WAVE_FORMAT_EXTENSIBLE` subformat. Frames
  are downmixed to mono by averaging the channels and normalised to `[-1, 1]`;
  8-bit PCM is read as unsigned, unlike every deeper depth. A file is refused
  past 256 MiB and past 32 channels, and the frames are cut at the ceiling the
  caller passes, with `source_frames` and `truncated` reporting what was cut.
  All of it runs on the control thread.
- The ring and the pool, in `src/core/sample.rs`. `MAX_SAMPLE_SLOTS` is 8,
  `MAX_SAMPLE_FRAMES` is 480_000 per slot (about ten seconds at 48 kHz) and
  `SAMPLE_RING_CAPACITY` is 65 536 messages. `SamplePool::new` allocates every
  slot up front, and both ways in write only into that memory: `push`, one message
  at a time, which is the path the audio thread takes, and `drain`, a whole ring
  at once. A slot stays unreadable until its `End` arrives, so half a sample can
  never play.
- The handoff, in `src/core/engine.rs`. `AudioEngine` owns both rings. Commands
  go through `send`; the sample ring's producer is handed out once by
  `take_sample_producer`, which `App::with` calls at startup, and the audio thread
  drains the ring into the pool before it applies commands, so a load pushed
  a moment ago is audible in the same callback. `set_blocks` pushes one lane's
  mirror. `resync` rebuilds every mirror from the song, which gives each block a
  span and a gain but no slot; `App::tick` follows it with `repush_lanes`, which
  sends every lane's real mirror, so an audio block keeps sounding across a
  resync.
- Loading, in `src/app/samples.rs`. `ImportSample { track, path, start_beats }`
  decodes with `MAX_SAMPLE_FRAMES`, takes the lowest free slot and pushes `Begin`,
  one `Frame` per sample and `End`. A file already loaded shares its slot. A full
  slot table, an unreadable file or a ring with no room is reported in the status
  line instead, and a refused load gives its slot back. The block's length is the
  sample's duration in beats at the current tempo, its name is the file stem, and
  a MIDI track becomes an audio track. `peaks` reduces the frames to
  `WAVEFORM_BUCKETS` (256) amplitudes for the browser's thumbnail and the block.
- The voice, in `src/core/synth.rs`. A track carries one `SamplePlayer` (slot, an
  `f64` position, a step, a gain, the tag of the block that started it, and a
  release level) beside its oscillator voices. `play_sample` refuses an unknown
  track, `NO_SLOT`, a slot whose load has not finished and a gain that is not
  positive; a new trigger replaces the voice already playing. `RenderCore`
  starts a block whose start falls in the buffer the playhead is about to render,
  at `EVENT_PITCH` (255, a pitch no keyboard sends) and the frame offset from
  6.1, and it releases the voice when the playhead passes that block's end.
- Cutting, not guessing. A sample longer than its block is cut at the block end
  with a short fade (`release_sample`, `RELEASE_FADE_FRAMES`), and the tag is what
  keeps one block's end from silencing the sample a later block started. A sample
  shorter than its block simply ends. The amplitude is the instrument's gain:
  `sample_gain` takes no block length, because an earlier formula that derived an
  amplitude from the block length drove almost every real block to zero and
  silenced the sample it was supposed to shape.
- Pitch, not time. `set_sample_rate` stores each slot's ratio of file rate to
  engine rate and the voice steps by it, so a 44.1 kHz file in a 48 kHz session
  runs slightly fast and high. That is repitch, Ableton Live's default, and the
  engine does it on purpose. Time-stretching, which would keep the pitch, does
  not exist; see 6.6.

The red-zone rule holds where the sample path touches it. The file read, the
decode and the pool allocation are all on the control thread, and the callback's
share is to pop `SampleMsg` values off the ring, copy them into memory the pool
already owns and read it: no lock, no I/O, no panic, no allocation. The only
route for audio frames into the audio thread is that ring. The contract is stated
in `src/core/mod.rs` and enforced in debug builds by `tests/test_rt_no_alloc.rs`.

What does not work:

- One sample voice per track. Two audio blocks that overlap on one lane cannot
  both sound: the later trigger replaces the earlier voice.

### 6.5 The sidebar: file browser, VST pane, effects pane

The window is two areas. The sidebar (`app/src/lib/components/Sidebar.svelte`) is
three tabs, one visible at a time and all three kept mounted:

- the file browser, `FileBrowser.svelte`
- the VST pane, `VstPane.svelte`
- the effects pane, `EffectsPane.svelte`

The main area is `Timeline.svelte`: the ruler with the zoom controls, one
`TrackHeader` and one `TrackLane` per track, one `Block` per block and the
`Playhead`, all in one scroll surface whose header column and ruler stay put.
`Transport.svelte` sits above and `StatusBar.svelte` below. Every piece of state
lives in one store, `app/src/lib/session.svelte.ts`, which polls the engine once
per animation frame, keeps the snapshot every component reads and collapses fader
and slider drags to one message per frame. The block drags and the drop from the
file browser are their own controllers, in `drag.svelte.ts` and
`sampleDrag.svelte.ts`, and the timeline geometry comes from `geometry.ts`.

The file browser is the only part of the interface that touches a path, and it
does so through the Tauri shell rather than the engine. The shell's commands, one
wrapper each in `app/src/lib/api.ts`:

| Command | What it returns |
|---------|-----------------|
| `samples_root` | `$CASSETTE_SAMPLES_DIR` (or legacy `$ZBOUB_SAMPLES_DIR`), else `~/Music` when it exists, else the home directory |
| `list_samples` | One directory: directories first, then names, at most 2 000 entries |
| `home_dir`, `parent_of` | The home directory and a path's parent, for the breadcrumbs |
| `read_waveform` | Peak amplitudes of an audio file, for the thumbnail and the block |

`list_samples` and `read_waveform` canonicalise the path they are given and
refuse anything outside the sample root, so the browser cannot walk out of it. An
audio file dragged onto a lane sends `ImportSample` with the beat under the pointer;
double-clicking one imports it into the selected track at the end of its
material.

The VST pane lists `UiState.presets`, the built-in voice names, and sends
`SetPreset`. It says in the interface that the engine hosts no plugin format. The
effects pane edits the selected track's voice, gain, octave and the ADSR
envelope, through `SetInstrument`, and says that insert effects are not
implemented. All three intents land in `src/app/`: `samples.rs` for the import,
`tracks.rs` for the voice.

What is not there: no plugin is hosted or scanned, and no insert effect exists to
put in the effects pane, so both panes describe the built-in voice instead. See
6.6.

The two commands the file browser depends on were invoked under the wrong names
until 2026-09-24: `app/src/lib/api.ts` called `parentOf` and `readWaveform`
while the shell registers `parent_of` and `read_waveform`. Tauri renames a
command's arguments, never the command's name, so both failed with "command not
found" and the browser caught it: the breadcrumb's up button did nothing and no
waveform was drawn. `api.ts` now invokes the two names the shell registers. Not
verified at runtime.

### 6.6 Still to do

Model and engine:

- Note data inside a MIDI block. `Block` has no notes, no clip and no content
  field, so a MIDI lane is silent and the synth is still played live from the
  keyboard.
- A richer project format. Save and load exist (JSON via `ExportProject` and
  `LoadProject`) but store the structure, tempo, blocks and sample paths only;
  there is no version field and no note data until Phase 7 adds it.
- Scenes and follow actions. Nothing groups blocks into scenes, and nothing
  triggers the next block when one ends; a block's material simply stops when the
  voice runs out.
- Plugin hosting. The `Processor` seam in 2.4 is still a plan. No VST, VST3, AU,
  CLAP or LV2 loader is in the crate, and the VST pane says so.
- Insert effects. The mixer is volume, pan, mute and solo. There is no effect
  slot on a track or on a block, and no signal path through one.
- A loop range and punch. Seeking works, but `looping` has no loop points to act
  on.
- Time-stretching. Nothing warps a sample to the project tempo: a sample at
  another rate plays pitch-shifted, which is repitch and Ableton Live's default.
  See 6.4.
- More than one sample voice per track. `TrackVoices` holds one `SamplePlayer`,
  so two audio blocks overlapping on one lane cannot both sound.

Interface:

- A piano roll or any other editor for MIDI block content (Phase 7).
- Pointer interactions (drag, resize, drop, import) were exercised by hand on
  2026-10-26; none has an automated test beyond `geometry.test.ts`.

---

## Phase 7: Note data in MIDI blocks (next)

A MIDI block holds notes (pitch, start, length, velocity) that the render core
starts as the playhead crosses them, then a piano roll to edit them. The
review below comes first: the fixes land before new features.

## Review findings (2026-10-26, v0.7.0)

Four parallel read-only reviews covered the engine, decoding and export, app
logic and project loading, and the frontend and Tauri shell. `cargo test`,
`clippy`, `fmt`, `svelte-check` and `bun test` were green. Findings are from
reading the code. The ones marked verified were re-read by hand.

Fixed on `fix/review-findings` (2026-10-26). Tests, clippy and fmt are green.
Not yet checked by ear or in a running window:

- Export now applies each sample's rate and no longer loops.
- The Load button opens the Tauri dialog and sends the chosen path.
- `SampleMsg::End` is bounds-checked, and the block mirror and sample `Arc`s are
  no longer freed on the audio thread.
- `load_project` validates the file (version field, clamps, limits, sorted
  blocks) and resets the previous project's samples, notes and cursor.
- Decoding stops at 10 minutes (`MAX_DECODED_FRAMES`) and reports truncation.
- Looping wraps the playhead at the end of the last block, and it is on by
  default.
- The beat position is computed in `f64`; seek and stop fade sample voices out.
- File browser commands run off the UI thread; keyboard import, stale-response
  guard, drag-drop cleanup, keyed mix queue and window drag permission landed.

Still open:

- `frames_per_beat` is rounded to an integer and drifts at non-integer tempos.
- The UI beat position in `src/app/mod.rs` still uses `f32`.
- Multi-file drops are staggered by a fixed bar, not by real block length.
- Loop points: the loop spans the whole song only.
- Dead code used only by tests (`block_at`, `contains`, `is_audible`, `lerp`,
  `subscriber_count`) and the `Begin`/`Frame`/`End` sample messages.
- The ffplay and TCP backends write on the render thread and can stall it.

---

## Key references

- [music-over-ssh.md](./docs/music-over-ssh.md): hearing the session when you are on SSH
- [ARCHITECTURE.md](./docs/ARCHITECTURE.md): layer separation, dependency rules
- [TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md](./docs/TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md): critical bug analysis
- [rust-daw-research.md](./docs/rust-daw-research.md): patterns from production Rust DAWs

## Dependencies

The engine crate `cassette` builds and tests with no GUI toolkit and no audio
headers.

| Crate | Role | Notes |
|-------|------|-------|
| `crossbeam` | Bounded MPSC channels | Event bus |
| `rtrb` | SPSC ring buffer | Interface → audio commands, and the sample ring |
| `anyhow` | Error handling at boundaries | |
| `serde` | Derives on the snapshot and the event vocabulary | |
| `assert_no_alloc` | Red-zone verification | Debug/minimal builds |
| `cpal` | Direct device audio | Optional, feature `cpal` |
| `symphonia` | Decoding of MP3, FLAC, M4A, AAC, OGG, Opus, AIFF and WAV | Pinned to 0.5.4 |

The Tauri shell `cassette-app` in `app/src-tauri/` adds `tauri` and
`tauri-build` for the window, `serde_json` to move events and snapshots over
IPC, plus `crossbeam` and `serde`. It enables the engine's `cpal` feature for
real device output. `tauri`, `tauri-build` and `serde_json` are shell-only;
`crossbeam` and `serde` are shared with the engine, and `serde` is also used by
`models`, `events` and `ui` for the wire types.

`ratatui` and `crossterm` were removed with the terminal interface.
