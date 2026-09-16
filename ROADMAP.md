# zboub — Roadmap

A terminal-based DAW (digital audio workstation) inspired by Ableton Live.
Keyboard-only, real-time audio engine separated from a ratatui terminal UI.

---

## Status

| Phase | Scope | Status |
|-------|-------|--------|
| Phase 1 | Critical fixes | Done |
| Phase 2 | Core audio engine | Done |
| Phase 3 | UI framework | Not started |
| Phase 4 | Polish and features | Not started |

---

## Phase 1 — Critical fixes (done)

All items completed and reviewed. See code changes in:
- `src/events/bus.rs` — fan-out with bounded crossbeam channels
- `src/core/audio.rs` — red-zone contract with forbid/expect and assert_no_alloc
- `src/main.rs` — global allocator registration
- `Cargo.toml` — crossbeam + assert_no_alloc + rtrb dependencies

---

## Phase 2 — Core audio engine

Build the real-time audio processing pipeline.

### 2.1 Sample-accurate transport

Track position in **frames** (not seconds) to avoid floating-point drift. Millisecond precision at 44.1 kHz = 44 frames.

- Transport position as `AtomicU64` (sample count), updated in the audio callback, read from the UI thread
- Loop points as `(start_frame, end_frame)` with an `Option` for no-loop
- BPM as `f32` stored in core; `sample_rate` decided by the audio backend
- `Play`/`Stop`/`Record` events carry frame offsets (not wall-clock times) so the audio thread can seek

### 2.2 Audio callback and backend trait

The audio callback closure must own everything it touches (mixer, buffers). Per the research doc, the pattern from whoisryosuke/daw-tauri applies: move the mixer into the closure at startup, no `Arc<Mutex>` needed.

Define a `AudioBackend` trait in `core/audio.rs`:

```rust
pub trait AudioBackend {
    fn sample_rate(&self) -> u32;
    fn start(&mut self, callback: Box<dyn FnMut(&mut [f32]) + Send>);
    fn stop(&mut self);
}
```

Implement for `cpal::Stream` (or stub a test backend). Core owns the `Box<dyn AudioBackend>`, the closure moves it into the audio thread.

### 2.4 Plugin hosting (optional, later)

`clack-host` (used by naiqvist/daw) or `nih-plug` for VST3/LV2 support. Insert a `Processor` trait object per track.

---

## Phase 3 — UI framework

Build the terminal presentation layer on ratatui.

### 3.1 Three-pane layout

- **Browser / Pattern Editor** — event-driven content browsing
- **Mixer Panel** — real-time visualization, track controls
- **Piano Roll** — MIDI input display and editing

### 3.2 Event-driven architecture

UI subscribes to the bus and redraws on every event or transport tick. Holds no mutable state owned by core — reads core state through shared, immutable snapshots.

### 3.3 Input adapter

`input::handler` is the only place raw terminal keycodes are read. Maps them through `input::keymap` into `Event` variants and pushes them onto the `EventBus`.

---

## Phase 4 — Polish and features

### 4.1 Offline rendering / bouncing

Export a rendered audio file from the session.

### 4.2 Multi-track recording

Record multiple simultaneous audio inputs.

### 4.3 Additional input devices

- MIDI keyboard via a new adapter in `input/`
- OSC control surface

### 4.4 Alternate renderers

Implement a new renderer that consumes `View` for headless testing or a different terminal UI.

---

## Key references

- [ARCHITECTURE.md](./docs/ARCHITECTURE.md) — layer separation, dependency rules, extension points
- [TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md](./docs/TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md) — critical bug analysis and implementation checklist
- [rust-daw-research.md](./docs/rust-daw-research.md) — patterns from production Rust DAWs (naiqvist/daw, sourdaw, generic-daw, Meadowlark, whoisryosuke/daw-tauri)

## Dependencies

| Crate | Role | Notes |
|-------|------|-------|
| `ratatui` | Terminal widget rendering | UI layer only |
| `crossterm` | Terminal event polling | Input adapter only |
| `anyhow` | Error handling at boundaries | |
| `rtrb` | SPSC ring buffer | Real-time safe audio comms |
| `crossbeam-channel` | Bounded MPSC channels | Event bus |
| `cpal` | Low-level audio I/O | Backend choice |
