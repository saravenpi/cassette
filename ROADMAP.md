# zboub: roadmap

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
| Phase 5 | Session view and blocks | In progress |

The engine crate builds with no webview and no audio headers, runs a session
track list, and plays the polyphonic synth from the keyboard. 81 tests pass
(62 unit tests in the lib, 7 engine integration, 10 MIDI/OSC adapter, 2
red-zone); `cargo clippy --all-targets` is clean.

The desktop app compiles and renders headlessly under Xvfb, but nobody has heard
it produce sound on real audio hardware. Treat "it makes noise" as unverified
until someone listens on a machine with a working output device.

The step sequencer, the step grid and the pattern model were removed in Phase 5:
the session view replaced them. Sections below marked superseded describe what
went away.

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
status footer instead; see 5.1.

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
`keys.ts` throws at load if a clash is ever introduced.

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
engine crate is a library, and `--headless` and `--stream-port` went with it. A
stale doc comment in `core/audio.rs` still names `--stream-port`. `ui/view.rs`
survives as the serializable `UiState` snapshot alone.

---

## Phase 5: Session view and blocks (in progress)

### 5.1 Track list replaces the step grid

The main view is a list of tracks. Each row carries its name, kind (MIDI or
audio), preset or waveform, its blocks, a `+` slot, a level meter, its volume
and pan, and mute and solo buttons. The step sequencer, the step grid, the
pattern model, the step clock and the performance recorder are gone.

### 5.2 Blocks

A block is a named slot holding no content yet. `N` or `enter` appends one,
`backspace` or `del` deletes the block under the cursor (and asks for a full
resync, because the audio-side mirror only appends), `R` renames the block under
the cursor, and left/right move between blocks. Notes for MIDI tracks and audio
files for audio tracks come later.

### 5.3 Transport

`Transport` survives the sequencer: it advances a frame counter and reports the
position in beats, published through `SharedState`. It schedules nothing, so a
project with no sounding notes is silent until you play the keyboard.

### 5.4 Still to do

Engine side. Reading and writing a song file, block content, block editing UI,
scenes and follow actions.

Interface side. No front-end exposes the audio-over-SSH streaming backend, even
though `AudioEngine::start_streaming` and `App::new_streaming` work and are
tested. The keymap and the UI have no test coverage: `keys.ts` guards against a
note/command clash at load, and nothing else checks it. The desktop app has
never been run against real audio hardware.

---

## Key references

- [music-over-ssh.md](./docs/music-over-ssh.md): hearing the session when you are on SSH
- [ARCHITECTURE.md](./docs/ARCHITECTURE.md): layer separation, dependency rules
- [TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md](./docs/TERMINAL_DAW_ARCHITECTURE_ANALYSIS.md): critical bug analysis
- [rust-daw-research.md](./docs/rust-daw-research.md): patterns from production Rust DAWs

## Dependencies

The engine crate `zboub69` builds and tests with no GUI toolkit and no audio
headers.

| Crate | Role | Notes |
|-------|------|-------|
| `crossbeam` | Bounded MPSC channels | Event bus |
| `rtrb` | SPSC ring buffer | Interface → audio commands |
| `anyhow` | Error handling at boundaries | |
| `serde` | Derives on the snapshot and the event vocabulary | |
| `assert_no_alloc` | Red-zone verification | Debug/minimal builds |
| `cpal` | Direct device audio | Optional, feature `cpal` |

The Tauri shell `zboub69-app` in `app/src-tauri/` adds `tauri` and
`tauri-build` for the window, `serde_json` to move events and snapshots over
IPC, plus `crossbeam` and `serde`. It enables the engine's `cpal` feature for
real device output. None of these appear in `core` or `models`.

`ratatui` and `crossterm` were removed with the terminal interface.
