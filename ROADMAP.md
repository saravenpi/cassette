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
| Phase 7 | Note data in MIDI blocks | Done (v0.9.0, 2026-10-26) |
| Phase 8 | Devices, metronome, quantize | Done (v0.14.0, 2026-10-27) |
| Phase 9 | Editing, arrangement, sound library | In progress (2026-10-27) |

The engine crate builds with no webview and no audio headers, plays the
polyphonic synth from the keyboard, and starts a sample when the playhead
reaches its block. The project is called cassette; it was zboub69 until v0.7.0.

Build state, measured on 2026-10-26 at v0.10.0: `cargo test` is green end to end
(206 library unit tests, 12 engine integration, 10 MIDI/OSC adapter, 5
red-zone, 8 wire), `cargo clippy --all-targets -- -D warnings` and
`cargo fmt --check` are clean, and `cd app && bun run check` and
`cd app && bun test` (122 tests) are clean.

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
| `F9` | arm MIDI recording |
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
| `P` | open the piano roll on the selected MIDI block |
| `esc` | close the piano roll |

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
- `end_beats` is `start_beats + length_beats`.
- `Track::blocks` is a `Vec<Block>` kept in start order. `insert_block` finds the
  slot with `partition_point` and returns the index, `add_block(start_beats,
  length_beats)` creates a block of the track's kind and names it with the
  block count, and `free_start()` is the ceiling of the last block's end, or 0 on an empty
  lane, which is where a new block lands.
- Nothing prevents two blocks from overlapping. The audio side starts both when
  the playhead crosses them.
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
holds no sample, and `start_blocks` skips it.

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
- The ring and the slots, in `src/core/sample.rs`. `MAX_SAMPLE_SLOTS` is 8. A
  `SampleMsg::Load` hands a whole decoded sample over as an `Arc<[f32]>` the
  sender keeps a clone of, and `Free` empties a slot. [SUPERSEDED by: v0.9.1,
  2026-10-26] The frame-by-frame `Begin`/`Frame`/`End` messages and the 15 MB
  preallocated pool they filled were used only by tests and are gone.
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
decode and the buffer allocation are all on the control thread, and the
callback's share is to pop `SampleMsg` values off the ring and swap a handle the
sender still holds: no lock, no I/O, no panic, no allocation, no free. The only
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
- Punch in and out. [SUPERSEDED by: v0.10.0, 2026-10-26] The loop range part:
  a shift-drag on the ruler now sets a loop region.
- Time-stretching. Nothing warps a sample to the project tempo: a sample at
  another rate plays pitch-shifted, which is repitch and Ableton Live's default.
  See 6.4.
- More than one sample voice per track. `TrackVoices` holds one `SamplePlayer`,
  so two audio blocks overlapping on one lane cannot both sound.

Interface:

- [SUPERSEDED by: Phase 7, v0.9.0, 2026-10-26] A piano roll or any other editor
  for MIDI block content.
- Pointer interactions (drag, resize, drop, import) were exercised by hand on
  2026-10-26; none has an automated test beyond `geometry.test.ts`.

---

## Phase 7: Note data in MIDI blocks (done, v0.9.0)

A MIDI block holds notes (pitch, start, length, velocity) that the render core
starts as the playhead crosses them, then a piano roll to edit them. The
review below came first: its fixes landed in v0.8.0.

Shipped in v0.9.0 as planned below, with these departures:

- The piano roll opens with `P`, not Enter: Enter already adds a block.
- The 60 Hz snapshot carries notes only for blocks on the selected track. A full
  session serialized to 2.1 MB per frame otherwise. One full track still costs
  that much; trimming to the selected block would cap it at about 33 KB. So the
  roll closes when the cursor leaves its track, and the note preview inside a
  block shows only on the selected track.
- A loop wrap releases every synth voice, keyboard notes included.
- Nobody has heard notes play from the timeline or used the roll in a running
  window yet.

The v0.10.0 review reversed two of those departures. [SUPERSEDED by: v0.10.0,
2026-10-26] The snapshot trim: each track now carries a `notesRevision`, the
webview sends back the revisions it holds, and the `state` command leaves those
tracks' notes out. Every block shows its notes again, and an idle frame carries
no notes. The render core also cuts each buffer at the next note start and
sample block edge, so notes and samples start on their own frame instead of up
to one buffer early.

### Plan (2026-10-26)

Decisions, all reversible before work starts:

- Storage: a flat `Vec<Note>` per MIDI block, in insertion order. Insertion
  order keeps a note's index stable while a drag moves it; sorting happens only
  when the mirror is built.
- A `Note` is `pitch: u8`, `start_beats: f32` (relative to the block start),
  `length_beats: f32`, `velocity: u8`. Defaults: velocity 100, length 1 beat,
  grid 1/4 beat.
- Project files keep `version: 1`. `Block.notes` is `#[serde(default)]`, so
  v0.7.0 files load and old builds ignore the field. `sanitize` clamps notes.
- The audio thread gets notes through a second mirror, one flat array per
  track, with absolute beats (block start plus note start), clipped to the block
  end and sorted. New `RtCommand::SetNotes` carries an `Arc<[MirroredNote]>` and
  reuses the `retired` list in `AudioEngine`, so the audio thread never frees it.
- Limits: `MAX_NOTES_PER_BLOCK = 512`, `MAX_NOTES = 2048` per track mirror.
- Playback: a note starts when its start falls in the buffer window, with a gate
  of `length_beats * frames_per_beat`. Same resolution as block starts (up to
  one buffer early). A seek into the middle of a note does not enter it. A loop
  wrap and a stop release every synth voice.
- Not in scope: MIDI input recording, quantize commands, loop points, the
  sample-message cleanup.

Work packages. Step 0 runs alone and lands the shared contract. Steps A, B and C
then run in parallel worktrees with disjoint files and merge into
`feat/midi-notes`.

| Step | Owner files | Deliverable |
|------|-------------|-------------|
| 0 contract | `src/models/note.rs` (new), `src/models/track.rs`, `src/models/mod.rs`, `src/events/types.rs`, `src/ui/view.rs`, `src/core/mod.rs`, `src/core/render.rs` (type and variant only), `src/core/engine.rs`, `app/src/lib/wire.ts` | `Note`, `Block.notes`, `NoteView`, `MirroredNote`, `RtCommand::SetNotes`, `AudioEngine::set_notes`, the five note `Event`s (`AddNote`, `MoveNote`, `ResizeNote`, `SetNoteVelocity`, `DeleteNote`) and their TS twins. Compiles, no behavior. |
| A engine | `src/core/render.rs`, `src/core/synth.rs`, `tests/test_rt_no_alloc.rs`, `tests/test_engine.rs` | `RenderCore` stores the note mirror in fixed arrays and starts notes in `begin_span`. Wrap, seek and stop release voices. Tests: note fires at the right frame, gate length, loop wrap cuts a note, no allocation in the callback. |
| B app | `src/app/notes.rs` (new), `src/app/mod.rs`, `src/app/timeline.rs`, `src/app/export.rs`, `src/models/track.rs` (`sanitize` only), `src/ui/view.rs` (snapshot fill) | Handlers for the five events with bounds and limit checks, `mirror_notes` and `push_notes` after every edit and in `repush_lanes`, export renders notes, project save and load round-trip, snapshot carries notes. Tests per handler, a v0.7.0 file loads, an exported WAV is non-silent at the note's time. |
| C frontend | `app/src/lib/components/PianoRoll.svelte` (new), `app/src/lib/pianoRoll.ts` and `.test.ts` (new), `app/src/lib/components/Block.svelte`, `app/src/routes/+page.svelte`, `app/src/lib/session.svelte.ts`, `app/src/lib/keys.ts` | Piano roll pane under the timeline, opened by double-click or Enter on a MIDI block. Click adds, drag moves and resizes, wheel changes velocity, Delete removes. Clicking a key auditions through `noteOn` and `noteOff`. MIDI blocks draw a mini note preview. Pure geometry in `pianoRoll.ts` with tests. |

Merge and gate (owner: this session, after A, B and C land):

1. Merge A, then B, then C. Resolve conflicts in `render.rs` and `view.rs`.
2. `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
   `cd app && bun run check && bun test`. All green.
3. Update this file, `CHANGELOG.md` and `README.md`. Cut no release until the
   user has listened to it.

Risks: the 60 Hz snapshot now carries notes (512 per block, 64 blocks), so
Step B measures the serialized size and trims to the selected block if it hurts.
The piano roll cannot be exercised by an automated test beyond the geometry
helpers, so it needs a hand pass in the running window.

## Phase 9: Editing, arrangement, sound library (done, v0.15.0)

### UX review (2026-10-27)

Findings against Ableton Live 12, ordered by cost to the user:

1. The piano roll resizes notes from the right edge only. It holds one
   selected note at a time, and a click on empty grid adds a note, so no
   marquee drag can start. It has no arrow-key nudge, no select all, and no
   copy, cut, paste or duplicate.
2. The arrangement has no time selection, no clipboard and no "loop the
   selection". It also cannot add, delete, duplicate or rename tracks: a song
   is always the four tracks it started with.
3. Save and Export write to a default path without asking. There is no Save
   As, no `Ctrl+S`, no unsaved-changes mark and no file type of our own.
4. The sidebar tab named "VST" lists the built-in voices. No VST is hosted.
5. Leftovers from v0.14: each effect kind may appear once per track, the
   instrument has no bypass, and the compressor cannot listen to another track.

Sources: https://www.ableton.com/en/live-manual/12/arrangement-view/ and
https://www.ableton.com/en/live-manual/12/editing-midi/

### Decisions

- Piano roll, following Live: dragging over empty grid draws a marquee. A
  click on a note selects it, and Shift+click adds it to the selection or takes
  it out. Double-click on empty grid adds a note. Draw mode (the `B` key while
  the roll has focus, or a pencil button) restores click-to-add. Dragging any
  selected note moves the whole selection. Dragging a left or right edge
  resizes every selected note from that edge. Arrows nudge by the grid step,
  Shift+Up and Shift+Down move by an octave, Shift+Left and Shift+Right change
  length, and `Ctrl+A`, `Esc`, `Delete`, `Ctrl+C`, `Ctrl+X`, `Ctrl+V` and
  `Ctrl+D` do the usual. Quantize applies to the selection, or to every note
  when nothing is selected. Batch edits travel as `AddNotes`, `EditNotes`
  (absolute values, one undo step per drag) and `DeleteNotes`.
- Arrangement, following Live: a click on empty lane places the marker, and a
  drag across lanes draws a time selection over a run of tracks. Shift+click
  extends it. Clicking a block selects that block's span. `Ctrl+X`, `Ctrl+C`,
  `Ctrl+D` and `Delete` act on the selection. `Ctrl+V` pastes at the marker on
  the selected track. `Ctrl+L` loops the selection. The same commands are in a
  context menu on the selection. The engine splits blocks at the range edges,
  so a range edit is exact and is one undo step.
- Tracks: Add audio track (`Ctrl+T`), Add MIDI track (`Ctrl+Shift+T`), Delete,
  Duplicate, and Rename (double-click the name). The limit is 16 tracks.
- Project files: the extension is `.cassette` (JSON inside), and `.json` still
  loads. Save asks for a path the first time, and later saves go to that path.
  Save As (`Ctrl+Shift+S`), Open (`Ctrl+O`) and Save (`Ctrl+S`) are on the
  keyboard. Export asks where to write the WAV. The transport shows the
  project name, with a dot when there are unsaved changes.
- Effects: six new kinds (Chorus, Phaser, Auto Pan, Redux, Utility, Limiter).
  Up to 8 effects per track, any kind more than once. The control thread builds
  each new effect's state and hands it to the audio thread over a ring. Removed
  state comes back over a return ring to be dropped, so the callback never
  allocates or frees. Each effect carries a stable `id`, so a reorder keeps its
  tail.
- Sidechain: the compressor's detector can listen to another track's signal,
  taken after that track's effects and before its fader. It uses the previous
  frame's value, so track order does not matter, and a muted kick still pumps
  the bass.
- Instruments: the voice gains unison and detune, a sub oscillator, a
  resonant low-pass with an envelope sweep, a pitch sweep for drums, and
  two-operator FM. New presets: Supersaw, Sub Bass, Acid, Keys (FM electric
  piano), Bell, Organ, Strings, Brass, 808, Tom, Clap, Rim. The instrument
  card gets an activator and Tone dials (cutoff, resonance, detune, sub). The
  sidebar tab becomes Instruments, with presets grouped by role.

### Work packages (parallel worktrees, disjoint files)

| Step | Owner files | Deliverable |
|------|-------------|-------------|
| A engine | `src/core/**`, `src/models/instrument.rs` (presets), `tests/test_rt_no_alloc.rs`, `tests/test_engine.rs` | Effect handoff and return rings keyed by id, the six new effects, sidechain, the voice features and presets, the instrument bypass. Allocation-free under `assert_no_alloc`. |
| B app | `src/app/**`, `src/models/{effect,track,song,note}.rs` | Range clipboard ops, track add, delete, duplicate and rename, batch note events, quantizing a subset, effect ids and duplicates, sidechain and instrument events, project path, dirty flag and `.cassette`. |
| C piano roll | `app/src/lib/components/PianoRoll.svelte`, `app/src/lib/pianoRoll.ts`, `app/src/lib/noteSelection.ts` (new), tests | Everything in the piano roll bullet above. |
| D arrangement | every other file under `app/src/**`, `app/src-tauri/**` | Time selection and its commands, track management, the save, open and export dialogs and `.cassette`, the Instruments tab, the instrument activator and Tone dials, the sidechain picker on the compressor. |

### Departures in implementation

- Held keyboard notes are preserved across loop wraps using `release_timeline_notes()` instead of cutting all synth voices.
- Piano roll zoom scale has dedicated bounds (`ROLL_ZOOM_MIN = 16`, `ROLL_ZOOM_MAX = 2048`), allowing deep inspection and editing of micro-timings.
- Right-clicking empty timeline space below tracks opens a context menu to add Audio or MIDI tracks.
- Sidechain compressor detector reads the source track's post-chain pre-fader signal from the previous frame.

## Phase 8: Devices, metronome, quantize (done, v0.14.0)

Shipped as planned below, with these departures:

- The engine preallocates every effect kind on every track: about 1.5 MB per
  track, mostly the 4-second delay buffer, so about 24 MB for 16 tracks at
  44.1 kHz.
- Muted and non-solo tracks still run their chains and discard the output, so
  a tail does not jump back when a track is unmuted.
- `Shift+Tab` switches views only when focus is not on a control, so reverse
  tabbing still works.
- The Instrument card has no activator: no event bypasses the voice.
- Nobody has heard the effects, the metronome or the count-in in a running
  window yet.

### Goal

Ableton's detail view under the timeline, with two views: the Clip view (the
piano roll) and the Device view (the selected track's instrument and insert
effects, left to right). The view follows the selection. Selecting a track shows
its devices. Opening a MIDI block shows its clip. Six built-in insert effects run
in the engine. A metronome with count-in and quantize, both post-record and
record-time, complete the recording loop from v0.14.0.

### UX, modelled on Ableton Live 12

Source: https://www.ableton.com/en/live-manual/12/working-with-instruments-and-effects/

- The detail pane sits under the timeline. A header strip holds a `Clip` and a
  `Devices` selector, the track or block name, and a hide button. `Shift+Tab`
  switches between the two views. The pane's top edge drags to resize it. The
  height persists per viewer in `localStorage`.
- Clicking a track header, or a block on it, selects the track. If the pane is
  open on Devices, it shows that track's chain at once. Opening a MIDI block
  (`P` or a double-click) switches to Clip.
- The chain reads left to right, in signal order. On a MIDI track the first
  device is the instrument (today's voice editor). On an audio track it is the
  sample source, with a short note. Next come the insert effects in order, then
  a drop zone: "Drop an effect here or pick one".
- Each device is a card. Its title bar holds the activator (on/off; an off
  device passes audio through untouched), the name, fold (double-click the title
  bar) and a delete button. Dragging the title bar reorders the chain. A context
  menu offers Move left, Move right, Fold and Delete. `Backspace`/`Delete`
  removes the device whose title bar has focus.
- Parameters are dials. Drag up or down to change one (`Shift` for fine steps),
  scroll over it, or use the arrow keys while it has focus. Double-click resets
  it to the default. A dial carries `role="slider"` and `aria-valuetext` with the
  unit. Choice parameters, like the filter type, are segmented buttons.
- The sidebar's Effects tab becomes the effect browser: one row per built-in
  effect. A double-click adds it to the selected track, and a drag drops it at a
  slot in the chain.
- The transport gets a metronome toggle and a menu button next to it. The menu
  sets the count-in (none, 1, 2 or 4 bars) and record quantize (off, 1/4, 1/8,
  1/16). While counting in, the position readout shows the bars left (`-2`,
  `-1`).
- Quantize: `Ctrl+U` (`Cmd+U` on macOS) quantizes the open clip, or the selected
  MIDI block, to the current grid. The piano roll bar has a Quantize button.

### Built-in effects

Each kind appears at most once per track. The engine preallocates one instance
of every kind per track, so the callback never allocates, and a chain is an
ordered list of kinds with their parameters. Adding a second instance of a kind
is refused with a status line. Lifting that limit would need state buffers
handed over a ring and retired off the audio thread. That is a later step.

| Kind | Parameters |
|------|------------|
| EQ Three | Low, Mid, High (dB), Low freq, High freq |
| Filter | Type (low-pass, high-pass, band-pass), Cutoff, Resonance |
| Saturator | Drive, Output, Mix |
| Compressor | Threshold, Ratio, Attack, Release, Makeup |
| Delay | Time (beats, tempo-synced), Feedback, Mix |
| Reverb | Size, Damping, Mix (Freeverb: 8 combs, 4 allpasses, stereo spread 23) |

Reverb reference: https://ccrma.stanford.edu/~jos/pasp/Freeverb.html

Signal path per track: voices plus the sample voice give a mono signal. It is
duplicated to stereo and runs through the chain. Then come the track volume, the
instrument gain and the balance pan, as today. The meter reads after the chain.
`process_frame` keeps rendering while any chain still rings, so tails survive
the last voice.

### Contract (step 0)

- `src/models/effect.rs`: `EffectKind`, `Effect { kind, enabled, params: [f32; MAX_PARAMS] }`,
  `EffectChain` (fixed array, `Copy`), `ParamSpec`, `EffectKind::spec()`.
- `Track.effects: Vec<Effect>`, `#[serde(default)]`, so older projects load.
- Events: `AddEffect`, `RemoveEffect`, `MoveEffect`, `SetEffectEnabled`,
  `SetEffectParam`, `QuantizeBlock`, `ToggleMetronome`, `SetCountIn`,
  `SetRecordQuantize`.
- `RtCommand`: `SetEffects { track, chain }`, `SetMetronome { on }`,
  `PlayAfterCountIn { beats }`.
- `SharedState.count_in_frames`. In `UiState`: `metronome`, `count_in_bars`,
  `count_in_beats_left`, `record_quantize_beats`, `effect_catalog`, and
  `TrackView.effects`.
- `wire.ts` gets the twins of all of the above.

### Work packages (parallel worktrees, disjoint files)

| Step | Owner files | Deliverable |
|------|-------------|-------------|
| A DSP | `src/core/effects.rs` (new), `src/core/synth.rs`, `tests/test_rt_no_alloc.rs` | The six effects and the per-track chain, tempo-synced delay, tails, meter after the chain. Tests per effect, plus no allocation with every effect on. |
| B transport | `src/core/render.rs`, `src/core/audio.rs`, `src/core/engine.rs`, `tests/test_engine.rs` | A metronome click (accent on the downbeat) mixed after the master, never in export. Count-in pre-roll, then play. `RenderCore::new` and `resync` push effect chains and tempo. Tests: a click lands on the beat frame, count-in delays the transport. |
| C app | `src/app/effects.rs` (new), `src/app/notes.rs`, `src/app/transport.rs`, `src/app/history.rs`, `src/app/mod.rs`, `src/app/export.rs`, `src/models/effect.rs`, `src/models/track.rs` | Effect edit handlers with limits, one undo step per edit, coalesced param drags. Quantize a block. Record quantize. Arming from stop counts in, and notes before the downbeat are not recorded. Metronome state. Export renders chains. Project round trip. |
| D frontend | `app/src/**` | The detail pane, the device chain, dials, the effect browser, transport metronome and menu, quantize button and `Ctrl+U`, `Shift+Tab`, pure helpers with `bun test` coverage. |

Gate after merging A, B, C, D in that order: `mise run check`, `mise run test`.
Then update the README, the changelog and this file, and ship v0.14.0.

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

Closed since (checked against the code on 2026-10-26):

- `frames_per_beat` is fractional (`utils::time::Tempo`), and the UI beat
  position is computed in `f64` before it is cast for the wire (v0.8.0).
- Multi-file drops wait for each block's real end (`dropQueue.ts`, v0.8.0).
- The ffplay and TCP backends write through a bounded queue on their own thread
  (v0.8.0).
- Loop points: a shift-drag on the ruler sets a loop region, saved with the
  project (v0.10.0).
- The `Begin`/`Frame`/`End` sample messages and their 15 MB pool are gone; a
  sample only arrives whole, as an `Arc` (v0.10.0).

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
