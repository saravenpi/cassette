# Architecture — Terminal DAW

A keyboard-only, terminal-based DAW inspired by Ableton Live. The design
prioritises a hard separation between the real-time audio engine and the
text user interface, with all cross-boundary communication flowing through
small, typed events on bounded queues.

---

## 1. Source directory map

```
src/
├── main.rs                  # Entry point; dispatches TUI / --headless
├── lib.rs                   # Crate root; re-exports the public API
├── app.rs                   # Orchestrator: terminal, event loop, commands
│
├── core/                    # Audio engine, transport, rendering
│   ├── synth.rs             # Polyphonic voices, oscillators, ADSR
│   ├── render.rs            # RenderCore + Transport; applies RtCommand
│   ├── audio.rs             # AudioBackend impls + SharedState atomics
│   ├── engine.rs            # Control side: command producer + shared state
│   └── meter.rs             # Level meters (Phase 4.2)
│
├── ui/                      # Thin terminal presentation layer
│   ├── mod.rs               # Module declarations
│   ├── view.rs              # UiState + View (plain-text renderer)
│   ├── layout.rs            # Pane focus model
│   └── renderer.rs          # ratatui three-pane drawing
│
├── events/                  # Cross-boundary messaging
│   ├── types.rs             # Small, typed Event enum
│   └── bus.rs               # Fan-out EventBus (bounded crossbeam)
│
├── input/                   # Hardware → Event adapter
│   ├── mod.rs               # MIDI / OSC adapters (Phase 4.3)
│   ├── handler.rs           # Reads raw key events, forwards to keymap
│   └── keymap.rs            # Key binding table (piano + commands)
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

docs/
└── ARCHITECTURE.md          # This file
```

---

## 2. Dependency direction

Dependencies point **inward** toward `core`. No inner module ever depends
on a outer one.

```
input ──► events ──► core ◄── ui
                          ▲
                        models   (shared data only)
```

| Layer | Allowed dependencies | Forbidden |
|-------|---------------------|-----------|
| `core` | `events::types`, `models`, `utils` | `ui`, `input` |
| `ui` | `core`, `events`, `models`, `utils` | `input` |
| `input` | `events`, `core` (read-only types) | `ui` |
| `utils` | nothing external | — |
| `models` | nothing external | — |

---

## 3. Runtime event flow

```
Keyboard ──► InputHandler ──► EventBus ──┬──► UI (pulls on idle/frame)
             (adapter)       (mpsc)      │
                                         └──► Core (consumes on tick)
```

### 3.1 Input adapter

`input::handler` is the **only** place raw terminal keycodes are read.
It maps them through `input::keymap` into `Event` variants and pushes
them onto the `EventBus`. The handler knows nothing about rendering.

### 3.2 Event bus

`events::bus` wraps an `std::sync::mpsc::channel`. Producers (input,
scheduled timers) call `send`; consumers (UI, engine) `recv` or
`try_recv`. The channel is **bounded** (fixed capacity) so a fast
producer never starves the audio thread — excess events are dropped
rather than buffered indefinitely.

### 3.3 UI loop (event-driven)

The app subscribes to the bus, drains events once per frame, applies them, and
renders a fresh `UiState`. It holds no mutable state owned by core — it reads
transport and meters through the atomics in `core::audio::SharedState`.
`view.rs` holds the logical screen model; `renderer.rs` translates it into
ratatui widgets.

### 3.4 Audio callback (pull-driven, real-time safe)

The audio thread pulls samples from core on a fixed schedule. It must
**never**:

- Allocate after startup (pre-allocate all buffers in `AudioBuffer`)
- Acquire locks (locks held across the audio callback are forbidden)
- Perform I/O (no logging, no file, no network)
- Call into any UI code (no direct reference to `ui::` from `core::`)

It reads only from ring buffers or lock-free queues filled by the engine
tick, and writes to the output device.

---

## 4. Core principles

1. **UI / core independence** — `core` has zero knowledge of the
   terminal. Swap ratatui for any renderer (or none, for headless tests)
   without touching a single line in `core`.

2. **Input as adapter** — `input::` translates device-specific
   keycodes into domain events. Adding a MIDI keyboard means a new
   adapter in `input/`, not changes in `core/` or `ui/`.

3. **Small typed events** — Every event in `events::types::Event` is a
   discrete action (`KeyPress`, `Play`, `NoteOn`, …). No envelopes, no
   raw byte streams. The bus carries intent, not presentation.

4. **Thin TUI** — `ui/` contains only presentation logic: layout
   calculation, widget trees, and drawing. All decisions ("is this clip
   playing?", "what is the BPM?") are answered by reading core state,
   never stored redundantly.

5. **Bounded queues everywhere** — All channels have a fixed
   capacity. Back-pressure is handled by dropping, not growing. This
   keeps tail latency predictable for the audio thread.

6. **Real-time audio safety** — The audio callback is a bare loop over
   pre-allocated buffers. No `Box`, no `Vec` push, no `Mutex`, no
   `println!`, no `EventBus` access.

---

## 5. Extension points

| Where | What | How |
|-------|------|-----|
| `input/` | New input device (MIDI keyboard, OSC) | Add a module implementing the same `handle_key → Event` contract |
| `core/audio.rs` | JACK / PipeWire backend | Replace the sink behind `AudioBuffer` with a JACK port writer; core is unaffected |
| `core/engine.rs` | Plugin hosts (VST/LV2) | Insert a `Processor` trait object per track; trait defined in `core` |
| `events/types.rs` | New event kinds | Add an enum variant; bus and all consumers handle it via `match` |
| `ui/` | Alternate renderer (TTY, websocket) | Implement a new renderer that consumes `View`, leave `view` and `layout` untouched |
| `models/` | Custom routing / effects | Extend model structs; `core` reads them through the same accessors |
| `utils/` | Domain utilities | Pure functions, no state — add freely |

---

## 6. Dependency summary (Cargo)

External crates used today:

| Crate | Role |
|-------|------|
| `ratatui` | Terminal widget rendering (UI layer only) |
| `crossterm` | Terminal event polling (input adapter only) |
| `anyhow` | Error handling at application boundaries |

None of these appear in `core` or `models`.
