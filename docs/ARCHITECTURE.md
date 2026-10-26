# Architecture

A session DAW inspired by Ableton Live. The design prioritises a hard separation
between the real-time audio engine and the interface, with all cross-boundary
communication flowing through small, typed events on bounded queues.

The interface is a SvelteKit frontend in `app/` behind a Tauri shell. That shell
is a client of the engine, not part of it: `core` still carries zero knowledge of
whatever draws the screen.

---

## 1. Source directory map

```
src/                         # the engine crate: cassette
├── lib.rs                   # Crate root; module declarations + public API
├── app/                     # Intent handling + the UiState snapshot
│   ├── mod.rs               # The App, its intent routing, and the snapshot
│   ├── timeline.rs          # Blocks: create, place, resize, delete, mirror
│   ├── tracks.rs            # Selection, kind, mixer strip, voice
│   ├── samples.rs           # Decode a file, take a slot, make a block
│   └── transport.rs         # Play/stop, tempo, rename prompt, held notes
│
├── core/                    # Audio engine, transport, rendering
│   ├── synth.rs             # Polyphonic voices, oscillators, ADSR, sample voice
│   ├── render.rs            # RenderCore + Transport; applies RtCommand
│   ├── sample.rs            # SamplePool + the SampleMsg ring (audio-side storage)
│   ├── wav.rs               # WAV decoder: PCM and float, downmixed to mono
│   ├── audio.rs             # AudioBackend impls + SharedState atomics
│   ├── engine.rs            # Control side: command producer + shared state
│   └── meter.rs             # Level meters
│
├── ui/                      # The presentation contract
│   └── view.rs              # UiState — the snapshot the webview draws
│
├── events/                  # Cross-boundary messaging
│   ├── types.rs             # Small, typed Event enum (the wire vocabulary)
│   └── bus.rs               # Fan-out EventBus (bounded crossbeam)
│
├── input/                   # Hardware → Event adapters
│   └── mod.rs               # MIDI / OSC adapters
│
├── models/                  # Domain data (no behaviour)
│   ├── song.rs              # Song: tempo, tracks
│   ├── track.rs             # Track: instrument, mixer, blocks, block geometry
│   └── instrument.rs        # Waveform + ADSR presets
│
└── utils/                   # Pure helpers
    ├── time.rs              # Tempo → frames
    └── frequency.rs         # MIDI note → Hz

app/                         # the desktop app
├── src/                     # SvelteKit frontend
│   ├── lib/wire.ts          # The wire contract: UiState, EngineEvent, samples
│   ├── lib/api.ts           # One function per Tauri command
│   ├── lib/keys.ts          # Keymap: piano row, commands, zoom keys
│   ├── lib/keyboard.ts      # The window listener that drives the store
│   ├── lib/geometry.ts      # Timeline geometry: beats to pixels, snapping, bars
│   ├── lib/palette.ts       # The eight track colours
│   ├── lib/*.svelte.ts      # The stores: session state, block drags, sample drags
│   ├── lib/components/      # Sidebar + panes, Timeline and its parts
│   └── routes/+page.svelte  # The window: transport, sidebar, timeline, status
└── src-tauri/
    ├── src/lib.rs           # Commands (state, send, sample browser) + thread
    └── src/main.rs          # Desktop entry point
```

---

## 2. Dependency direction

Dependencies point **inward** toward `core`. No inner module ever depends on an
outer one.

```
input ──► events ──► app ──► core
                      │        ▲
                      ▼        │
                  ui::view   models   (shared data only)
```

| Layer | Allowed dependencies | Forbidden |
|-------|---------------------|-----------|
| `core` | `events::types`, `models`, `utils` | `ui`, `input`, `app` |
| `ui` | `models` | `core`, `events`, `input`, `app` |
| `input` | `events` | `core`, `ui`, `app` |
| `app` | all of the above | — |
| `utils` | nothing external | — |
| `models` | nothing external | — |

`ui::view` depends on `models` alone. It holds no references into `core`, which is
what lets a serializable snapshot cross the IPC boundary at all.

---

## 3. Runtime event flow

```
                          ┌──► core (ring buffer → audio thread)
                          │
  webview ──Tauri──► app ─┤
  input/  ──EventBus─►    │
                          └──► UiState ──Tauri──► webview
```

### 3.1 The two ways in

The webview sends an `Event` as JSON through the Tauri `send` command. The MIDI
and OSC adapters in `input/` produce the same `Event` values. Both funnel through
`App::submit`, so there is exactly one path into the app and one vocabulary
describing intent.

`Event` is the wire contract: it derives `Serialize`/`Deserialize` with
`rename_all = "camelCase"`, and `app/src/lib/types.ts` mirrors it by hand. Nothing
else crosses the boundary.

### 3.2 Event bus

`events::bus` is a bounded fan-out over crossbeam channels. Producers call `send`;
each subscriber gets its own fixed-capacity channel, so a slow consumer cannot
stall a producer. Excess events are dropped rather than buffered indefinitely.

### 3.3 The engine thread

The engine owns a cpal `Stream`, and cpal marks `Stream` `!Send` and `!Sync` on
every platform, so the engine cannot live in Tauri's managed state, which must be
`Send + Sync`. The shell therefore gives it a thread of its own.

That thread wakes on a command or once per frame, whichever comes first:

1. drain the bus and apply each intent to the song and the engine, pushing the
   lane's block mirror whenever a timeline edit changed it,
2. rebuild every mirror with `AudioEngine::resync` when a command was refused,
3. expire held notes the keyboard never released,
4. read the meters from `core::audio::SharedState`,
5. publish a fresh `UiState` for the webview to read.

A command wakes the thread at once rather than on the next frame, so a note
sounds the moment the key goes down. The webview cannot draw faster than 60 Hz, so
that is the frame rate, and the thread sleeps in between rather than spinning.

### 3.4 Audio callback (pull-driven, real-time safe)

The audio thread pulls samples from core on a fixed schedule. It must **never**:

- Allocate after startup (every buffer is allocated at construction)
- Acquire locks (locks held across the audio callback are forbidden)
- Perform I/O (no logging, no file, no network)
- Call into any interface code (no direct reference to `ui::` from `core::`)

It reads only from the ring buffers the control thread fills, and writes to the
output device. Those are the `RtCommand` ring and the `SampleMsg` ring; this is
why the engine thread above owns the lock: nothing in the shell can take a lock
the audio callback needs.

---

## 4. Core principles

1. **Engine / interface independence** — `core` has zero knowledge of the
   frontend. The Tauri shell, the SvelteKit app, or a future terminal front-end
   can each be replaced without touching a single line in `core`.

2. **Input as adapter** — `input/` translates device-specific protocols into
   domain events. Adding a MIDI keyboard means a new adapter in `input/`, not
   changes in `core/` or `app.rs`.

3. **Small typed events** — Every variant of `events::types::Event` is a
   discrete action (`AddBlock`, `NoteOn`, …). No envelopes, no raw byte streams.
   The bus carries intent, not presentation.

4. **Thin interface** — `ui/view.rs` holds only a snapshot: values read from the
   song and from core's atomics, with no logic of its own. Every decision ("is
   this playing?", "what is the BPM?") is answered by the engine, never stored
   redundantly in the frontend.

5. **Bounded queues everywhere** — All channels have a fixed capacity.
   Back-pressure is handled by dropping, not growing. This keeps tail latency
   predictable for the audio thread.

6. **Real-time audio safety** — The audio callback is a bare loop over
   pre-allocated buffers. No `Box`, no `Vec` push, no `Mutex`, no `println!`, no
   `EventBus` access.

---

## 5. Extension points

| Where | What | How |
|-------|------|-----|
| `input/` | New input device (MIDI keyboard, OSC) | Add a translator producing `Event` values |
| `events/types.rs` | New intent | Add an enum variant; mirror it in `app/src/lib/types.ts` |
| `app/src-tauri/src/lib.rs` | New command | Add a `#[tauri::command]`; no engine change needed |
| `app/src/routes/` | Interface changes | Read the snapshot, send intent; never compute timing |
| `core/audio.rs` | JACK / PipeWire backend | Implement `AudioBackend`; core is unaffected |
| `core/engine.rs` | Plugin hosts (VST/LV2) | Insert a `Processor` trait object per track |
| `models/` | Custom routing / effects | Extend model structs; `core` reads them through the same accessors |
| `utils/` | Domain utilities | Pure functions, no state — add freely |

---

## 6. The timeline, the sidebar and the sample path

### 6.1 Two areas, one snapshot

The window is one sidebar and one main area. The sidebar holds the sample file
browser, the VST pane and the effects pane. The main area holds a beat ruler, one
lane per track, the blocks placed on those lanes, and the playhead.

`core` knows none of that. The split lives in the snapshot, which is the whole
interface contract:

| What the interface draws | Where it comes from |
|--------------------------|---------------------|
| The ruler and the grid | `UiState.length_beats`, `bars`, `beats_per_bar` |
| The playhead | `UiState.transport_position_beats` |
| One lane per track | `UiState.tracks`, one `TrackView` each |
| A block's rectangle | `BlockView.start_beats`, `length_beats` |
| A block's name and cursor state | `BlockView.name`, `selected` |
| The VST pane's menu | `UiState.presets` |

An audio block also carries `source`, the file it plays; a MIDI block leaves it
empty. The interface changes a block by sending intent (`CreateBlock`,
`PlaceBlock`, `ResizeBlock`, `DeleteBlockAt`) and never by writing a position
into the snapshot. `TrackView::end_beats` reports where a lane's material ends,
which is what the ruler has to cover.

The frontend is split along those lines. `app/src/lib/components/` holds
`Sidebar.svelte` (three tabs: `FileBrowser`, `VstPane`, `EffectsPane`),
`Timeline.svelte` (a `Ruler`, one `TrackHeader` and one `TrackLane` per track,
one `Block` per block, and the `Playhead`), `Transport.svelte` and
`StatusBar.svelte`, and `routes/+page.svelte` is those four pieces stacked. One
store, `lib/session.svelte.ts`, polls the engine once per animation frame, holds
the snapshot every component reads and funnels intent back. `lib/geometry.ts`
holds the beats-to-pixels maths, the snap grid and the bar maths and is pure;
`lib/drag.svelte.ts` drives the block drags, `lib/sampleDrag.svelte.ts` the drop
from the browser, and `lib/keyboard.ts` the window listener.

The sidebar's file browser is the one place the interface touches a path, and it
goes through the Tauri shell rather than the engine: `samples_root`,
`list_samples`, `home_dir`, `parent_of` and `read_waveform`. The shell
canonicalises every path and refuses anything outside the sample root, so the
browser cannot walk out of it. Nothing about a path reaches `core`.

### 6.2 The timeline in the engine

A song is one timeline measured in beats from the top, and every track owns a
lane on it. `src/models/track.rs` holds the geometry:

- `Block` carries `name`, `kind`, `start_beats`, `length_beats` and `source`.
  `MIN_LENGTH_BEATS` is 0.25, `DEFAULT_LENGTH_BEATS` is 4.0 and
  `MAX_LENGTH_BEATS` is 512.0; `set_start` and `set_length` clamp to those.
- Two blocks may overlap.
- `Track::blocks` is a `Vec<Block>` kept in start order by `insert_block`, and
  `free_start()` is where a new block lands without touching an existing one.

The audio thread never sees a `Block`. It sees `MirroredBlock { start_beats,
end_beats, slot, gain }`, pushed per track by `RtCommand::SetBlocks`, which
replaces that track's mirror outright rather than appending to it. Geometry
therefore crosses the boundary as numbers, and the audio side needs no model
lookup and no name. `src/core/render.rs` computes nothing about a block except
whether the playhead crossed its start, and `src/app/timeline.rs` builds the
vector from a lane and pushes it after any edit that changes that lane, so a drag
costs one small `Vec`.

Playback is driven by that crossing. `RenderCore::process` reads the transport
position before and after a buffer; when the transport is playing,
`start_blocks` starts the sample of every block whose start lies in that span,
entering it at the frame offset `(to_beats - start_beats) * frames_per_beat` so
its content stays in step with the playhead. A crossing is noticed at the end of
the buffer, so a block starts up to one buffer late.

Two things follow from the transport having no seek: `Stop` rewinds to frame 0,
and that is the only position move, so a song always plays from the top.

### 6.3 The sample path

Decoding, storage and playback all live in `core`, and they are split so that the
audio callback never allocates:

```
control thread                       audio thread (red zone)
──────────────                       ────────────────────────
file on disk
  │ core::wav::decode                reads and allocates
  ▼
SampleMsg { Begin, Frame, End }
  │ app::samples::push_load          into an rtrb ring
  ▼
                                     AudioThread::process pops the ring
                                     and pushes each message into the pool,
                                     whose slots were allocated at construction
                                       │
core::render::start_blocks             ▼
  play_sample(track, pitch, slot,    sample voice
              offset, gain)            │
                                       ▼
                                     track mixer, then master
```

- `core::wav` is a hand-written reader: PCM at 8, 16, 24 and 32 bits and IEEE
  float at 32 and 64, including the `WAVE_FORMAT_EXTENSIBLE` subformat, downmixed
  to mono and normalised to `[-1, 1]`. It refuses a file past 256 MiB and past
  32 channels, and reports truncation through `source_frames` and `truncated`.
- `core::sample` owns the audio-side storage. `SamplePool::new` allocates
  `MAX_SAMPLE_SLOTS` (8) slots of `MAX_SAMPLE_FRAMES` (480_000) up front, and both
  ways in write only into that memory: `push`, one message at a time, which is the
  path the audio thread takes, and `drain`, a whole ring at once. A slot only
  becomes readable once its `End` message arrives, so a partly loaded sample can
  never play, and `rate(slot)` keeps the rate the file was decoded at.
- `core::render::start_blocks` starts a sample voice for each crossed block,
  through `play_sample(track, EVENT_PITCH, slot, offset, gain)`. `gain` is the
  amplitude the voice starts at, the release tail for a block whose material
  runs past its end.

**The red-zone rule, where the sample path touches it.** `RenderCore::process`
runs inside the audio callback and never allocates, takes a lock, performs I/O
or panics. Everything on the sample path that does any of those things happens
before the callback: the disk read, the decode and the `SamplePool` allocation
are all on the control thread. The callback's share is to pop `SampleMsg` values
off the ring, copy them into memory the pool already owns, and read it back. The
only route for audio frames into the audio thread is that ring; nothing hands
the callback a slice, a `Vec` or a path. The contract is stated in
`src/core/mod.rs` and enforced in debug builds by `tests/test_rt_no_alloc.rs`.

The voice is `SamplePlayer` in `core::synth`, one per track, holding the slot, an
`f64` position inside the sample, a step, a gain, the tag of the block that
started it and a release level. `play_sample` starts it and the track's sample
voice is summed with its synth voices. `sample_gain` is the amplitude a voice
plays at, which is the instrument's gain: the block's length does not enter it.
What stops a sample longer than its block is `release_sample`, which the render
core calls when the playhead passes the block's end, cutting the voice with a
short fade. The tag is what keeps one block's end from silencing the sample a
later block started. `set_sample_rate` stores each slot's ratio of file rate to
engine rate, and the voice steps by it, so material at another rate plays
pitch-shifted. That is repitch, Ableton Live's default mode, and it is deliberate
here rather than a bug: time-stretching, which would hold the pitch, does not
exist.

The ring is sized to hold the largest load whole (`SAMPLE_RING_CAPACITY` is
`MAX_SAMPLE_FRAMES` plus a header and a tail), because the control thread pushes
a whole file in one go and one message carries one frame. A ring smaller than
that refuses every file past its capacity, which is how a 65 536-message ring
came to refuse every sample longer than about a second and a half.

`ROADMAP.md` 6.4 and 6.6 carry the rest of the sample path's status, including
what is missing.

---

## 7. Dependency summary (Cargo)

The engine (`cassette`) is deliberately light: it builds and tests with no GUI
toolkit and no audio headers.

| Crate | Role |
|-------|------|
| `crossbeam` | Bounded fan-out channels for the event bus |
| `rtrb` | Lock-free SPSC ring buffer, control thread to audio thread |
| `anyhow` | Error handling at application boundaries |
| `serde` | Derives on the snapshot and the event vocabulary |
| `assert_no_alloc` | Red-zone allocation guard, debug builds only |
| `cpal` | Optional: direct device output (`cpal` feature) |

The Tauri shell (`cassette-app`) adds `tauri`, `tauri-build` and `serde_json`, and
enables the engine's `cpal` feature. None of these appear in `core` or `models`.