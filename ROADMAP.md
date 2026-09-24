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
| Phase 5 | Session view and blocks | In progress |

The app builds, runs a session track list, plays the synth from the keyboard,
and streams audio over SSH. 93 tests pass; `cargo clippy --all-targets` is clean.

The step sequencer, the step grid and the pattern model were removed in Phase 5:
the session view replaced them. Sections below marked superseded describe what
went away.

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

Position is a frame counter advanced in [`core::render`]'s `Transport`,
published to the UI through `SharedState` atomics. Tempo changes re-scale the
frame phase so the musical position never drifts.

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

## Phase 3 — UI framework (done)

### 3.1 Session view

[SUPERSEDED by: Phase 5 session view, 2026-09-24] The Mixer (track list, meters,
mute/solo), Pattern (step grid with a beat ruler) and Instrument (waveform,
envelope, step pitch) panes were replaced by a track list with blocks plus an
Inspector pane. A transport bar and a status line sit either side; `?` opens a
help overlay.

### 3.2 Event-driven architecture

`input` pushes `Event`s onto the bus; the app drains them once per frame and
applies them, then renders a fresh `UiState` snapshot. The UI holds no state
owned by `core`.

### 3.3 Input adapter

`input::handler` is the only place raw terminal key codes are read.
`input::keymap` maps them into `Event`s, including a two-octave piano.

## Phase 4 — Polish and features (done)

### 4.1 Offline rendering / bouncing

[SUPERSEDED by: Phase 5 session view, 2026-09-24] `core::offline` and the `E`
bounce were removed with the step grid: with no note data to schedule, a bounce
would have written silence.

### 4.2 Level metering

`core::meter` provides smoothed `LevelMeter`s for the track list. The
`PerformanceRecorder` was removed with the pattern it wrote into; capturing live
notes needs block content, which does not exist yet.

### 4.3 Additional input devices

`input::MidiInputAdapter` and `input::OscInputAdapter` translate MIDI and OSC
into the shared event vocabulary. Hardware transport is not wired up yet; the
adapters and their tests are in place.

### 4.4 Alternate renderers

`ui::view::View` renders `UiState` to plain text. `cargo run -- --headless`
exercises it without a terminal or an audio device.

---

## Phase 5 — Session view and blocks (in progress)

### 5.1 Track list replaces the step grid

The main view is a list of tracks. Each row carries an arm dot, the track name,
its kind (MIDI or audio), its blocks, a `+` slot, a level meter and the volume.
The step sequencer, the step grid, the pattern model, the step clock and the
performance recorder are gone.

### 5.2 Blocks

A block is a named slot holding no content yet. `A` appends one, `D` deletes the
block under the cursor (and asks for a full resync, because the audio-side mirror
only appends), `enter` renames the block under the cursor, and left/right move
between blocks. Notes for MIDI tracks and audio files for audio tracks come
later.

### 5.3 Transport

`Transport` survives the sequencer: it advances a frame counter and reports the
position in beats, published through `SharedState`. It schedules nothing, so a
project with no sounding notes is silent until you play the keyboard.

### 5.4 Still to do

Reading and writing a song file, block content, block editing UI, scenes and
follow actions.

---

## Key references

- [music-over-ssh.md](./docs/music-over-ssh.md) — hearing the session when you are on SSH
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
