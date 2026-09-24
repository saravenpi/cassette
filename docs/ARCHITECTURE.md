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
src/                         # the engine crate: zboub69
├── lib.rs                   # Crate root; module declarations + public API
├── app.rs                   # Intent handling + the UiState snapshot
│
├── core/                    # Audio engine, transport, rendering
│   ├── synth.rs             # Polyphonic voices, oscillators, ADSR
│   ├── render.rs            # RenderCore + Transport; applies RtCommand
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
│   ├── track.rs             # Track: instrument, mixer, blocks
│   └── instrument.rs        # Waveform + ADSR presets
│
└── utils/                   # Pure helpers
    ├── math.rs
    ├── time.rs              # Tempo → frames
    └── frequency.rs         # MIDI note → Hz

app/                         # the desktop app
├── src/                     # SvelteKit frontend
│   ├── lib/api.ts           # The two commands (state, send)
│   ├── lib/keys.ts          # Keymap: piano row + commands
│   └── routes/+page.svelte  # The session view
└── src-tauri/
    ├── src/lib.rs           # Commands + the engine thread
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

1. drain the bus and apply each intent to the song and the engine,
2. resync the audio-side mirror if a slot ran out or a block was removed,
3. expire held notes the keyboard never released,
4. read the meters from `core::audio::SharedState`,
5. publish a fresh `UiState` for the webview to read.

A command wakes the thread at once rather than on the next frame, so a note
sounds the moment the key goes down. The webview cannot draw faster than 60 Hz, so
that is the frame rate, and the thread sleeps in between rather than spinning.

### 3.4 Audio callback (pull-driven, real-time safe)

The audio thread pulls samples from core on a fixed schedule. It must **never**:

- Allocate after startup (pre-allocate all buffers in `AudioBuffer`)
- Acquire locks (locks held across the audio callback are forbidden)
- Perform I/O (no logging, no file, no network)
- Call into any interface code (no direct reference to `ui::` from `core::`)

It reads only from ring buffers or lock-free queues filled by the engine
tick, and writes to the output device. This is why the engine thread above owns
the lock: nothing in the shell can take a lock the audio callback needs.

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

## 6. Dependency summary (Cargo)

The engine (`zboub69`) is deliberately light: it builds and tests with no GUI
toolkit and no audio headers.

| Crate | Role |
|-------|------|
| `crossbeam` | Bounded fan-out channels for the event bus |
| `rtrb` | Lock-free SPSC ring buffer across the audio boundary |
| `anyhow` | Error handling at application boundaries |
| `serde` | Derives on the snapshot and the event vocabulary |
| `assert_no_alloc` | Red-zone allocation guard, debug builds only |
| `cpal` | Optional: direct device output (`cpal` feature) |

The Tauri shell (`zboub69-app`) adds `tauri`, `tauri-build` and `serde_json`, and
enables the engine's `cpal` feature. None of these appear in `core` or `models`.