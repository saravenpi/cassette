# zboub — Roadmap

A terminal-based DAW inspired by Ableton Live. Keyboard-only, with a real-time
audio engine separated from a ratatui terminal UI.

---

## Status

| Phase | Scope | Status |
|-------|-------|--------|
| Phase 1 | Critical fixes | Done |
| Phase 2 | Core audio engine | Done |
| Phase 3 | UI framework | Done |
| Phase 4 | Polish and features | Done |

The app builds, runs, plays a demo pattern on launch, and bounces to WAV.
95 tests pass; `cargo clippy --all-targets` is clean.

---

## Phase 1 — Critical fixes (done)

- `src/events/bus.rs` — fan-out, bounded crossbeam channels; slow subscribers
  are skipped instead of stalling producers.
- `src/core/audio.rs`, `src/core/render.rs` — red-zone contract: no allocation,
  no locking, no I/O, no panics in the render path.
- `src/main.rs` — `assert_no_alloc` global allocator.
- `Cargo.toml` — `crossbeam`, `rtrb`, `assert_no_alloc`; optional `cpal`.

## Phase 2 — Core audio engine (done)

### 2.1 Sample-accurate transport

Position is a frame counter advanced in [`core::sequencer`], published to the UI
through `SharedState` atomics. Loop points are implicit (loop the whole pattern);
tempo changes re-scale the frame phase so timing never drifts.

### 2.2 Audio callback and backend trait

`core::audio::AudioBackend` has three implementations: `CpalBackend`
(feature `cpal`), `SubprocessBackend` (ffplay pipe) and `NullBackend`. The
render closure owns the synth and sequencer, so nothing is shared behind a lock.

### 2.3 Polyphonic synth and mixer

`core::synth` holds fixed arrays (`MAX_TRACKS × MAX_VOICES`) with per-track
volume, pan, mute and solo. Voices use a 5-waveform oscillator and an ADSR
envelope with precomputed per-sample increments.

### 2.4 Plugin hosting (optional, later)

Not started. The natural seam is a `Processor` trait behind `RtCommand`, with
`clack-host` or `nih-plug` implementing it.

## Phase 3 — UI framework (done)

### 3.1 Three-pane layout

Mixer (track list, meters, mute/solo), Pattern (step grid with a beat ruler)
and Instrument (waveform, envelope, step pitch) panes, plus a transport bar and
a status line. `?` opens a help overlay.

### 3.2 Event-driven architecture

`input` pushes `Event`s onto the bus; the app drains them once per frame and
applies them, then renders a fresh `UiState` snapshot. The UI holds no state
owned by `core`.

### 3.3 Input adapter

`input::handler` is the only place raw terminal key codes are read.
`input::keymap` maps them into `Event`s, including a two-octave piano.

## Phase 4 — Polish and features (done)

### 4.1 Offline rendering / bouncing

`core::offline` renders the song through the same `RenderCore` and writes a
16-bit PCM WAV. `E` in the app bounces two loops.

### 4.2 Recording

`core::recorder` provides smoothed `LevelMeter`s for the mixer and a
`PerformanceRecorder` that quantises live played notes into the pattern.

### 4.3 Additional input devices

`input::MidiInputAdapter` and `input::OscInputAdapter` translate MIDI and OSC
into the shared event vocabulary. Hardware transport is not wired up yet; the
adapters and their tests are in place.

### 4.4 Alternate renderers

`ui::view::View` renders `UiState` to plain text. `cargo run -- --headless`
exercises it without a terminal or an audio device.

---

## Key references

- [ARCHITECTURE.md](./docs/ARCHITECTURE.md) — layer separation, dependency rules
- [TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md](./docs/TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md) — critical bug analysis
- [rust-daw-research.md](./docs/rust-daw-research.md) — patterns from production Rust DAWs

## Dependencies

| Crate | Role | Notes |
|-------|------|-------|
| `ratatui` | Terminal widget rendering | UI layer only |
| `crossterm` | Terminal event polling | Input adapter only |
| `anyhow` | Error handling at boundaries | |
| `rtrb` | SPSC ring buffer | UI → audio commands |
| `crossbeam` | Bounded MPSC channels | Event bus |
| `assert_no_alloc` | Red-zone verification | Debug/minimal builds |
| `cpal` | Direct device audio | Optional, feature `cpal` |
