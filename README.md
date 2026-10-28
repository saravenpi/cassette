# cassette

A desktop DAW inspired by Ableton Live. Arrange MIDI and audio clips on a beat timeline, edit notes in the piano roll, inspect audio spectra and fades, shape sound through per-track and master effect chains, and mix in the timeline or vertical mixer view.

The Rust audio engine handles all timing, synthesis, sample playback, recording, and DSP on a lock-free real-time thread. The SvelteKit frontend runs inside a Tauri shell, sends typed events to the engine, and renders the state snapshot published each frame.

![Cassette DAW session view](website/static/screenshot-devices.png)

## Features

- **Timeline and Mixer views**: Arrange MIDI and audio clips across up to 16 tracks plus the Master bus, or switch to the vertical channel-strip mixer (`Tab`).
- **Clip editing**: Single-click any MIDI clip to edit notes in the Piano Roll with zoom-adaptive quantize (`Ctrl+U`), or single-click any audio clip to inspect its waveform, fade-in/fade-out handles, and real-time 36-band spectrum analyzer.
- **Audio and MIDI recording**: Arm any audio track (`●`) or select a MIDI track and press `F9` to record directly onto the timeline at the playhead.
- **Synthesizer and Effects**: Built-in polyphonic synthesizer with 32 presets, plus insert effects on every track and the Master bus (EQ Eight with live spectrum, Compressor with transfer curve and gain-reduction trace, Filter, Delay, Reverb, Distortion, Chorus, Gain, Limiter, Bitcrusher, Phaser, Tremolo, Gate, Utility).
- **Zero-allocation audio callback**: Commands and samples cross threads via lock-free `rtrb` ring buffers; `tests/test_rt_no_alloc.rs` enforces zero heap allocations on the audio render path.

## Quick start

[mise](https://mise.jdx.dev) manages the Rust and Bun toolchains and task scripts on Linux, macOS, and Windows:

```bash
mise install           # install pinned Rust and Bun toolchains
mise run deps          # install native Tauri system dependencies
mise run dev           # launch the desktop app with hot reload
mise run build         # build a release bundle for the current platform
mise run test          # run all Rust and frontend test suites
mise run check         # run rustfmt, clippy, and svelte-check
```

Without `mise`, install Rust, [Bun](https://bun.sh), and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/):

```bash
cd app
bun install
bun run tauri dev
```

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Space` | Play / Stop transport |
| `F9` | Record into armed/selected audio track or selected MIDI track |
| `L` | Toggle loop mode |
| `Ctrl+L` | Loop active time selection |
| `Tab` | Switch between Timeline and Mixer view |
| `Shift+Tab` | Switch detail pane between Clip and Devices |
| `Click clip` / `P` | Open Clip view (Piano Roll for MIDI, Audio Clip inspector for Audio) |
| `Ctrl+U` | Quantize note starts and lengths to the active grid |
| `B` | Split block at pointer (or toggle draw mode inside Piano Roll) |
| `Ctrl+E` | Split selected block at playhead |
| `N` / `Enter` | Insert MIDI block at playhead |
| `Ctrl+D` | Duplicate selected block, time range, or notes |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | Copy, cut, or paste selection, notes, or effects |
| `Backspace` / `Delete` | Delete selected block, time range, or notes |
| `Ctrl+T` / `Ctrl+Shift+T` | Add Audio track / Add MIDI track |
| `M` / `O` | Mute / Solo selected track |
| `R` | Rename selected block |
| `Z` / `X` | Shift computer keyboard MIDI octave down / up |
| `A W S E D F T G Y H U J K` | Play MIDI notes from computer keyboard |
| `-` / `=` | Adjust track volume (`+` / `-` in Piano Roll zooms roll) |
| `[` / `]` | Adjust track pan |
| `,` / `.` | Adjust tempo (BPM) |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / Redo |
| `Ctrl+S` / `Ctrl+Shift+S` | Save / Save As project (`.cassette`) |
| `Ctrl+O` | Open project |
| `Esc` | Close active modal, clear selection, or close clip view |

## Audio backends

Select output and input devices directly in **Settings -> Audio**. The engine supports four backends:

1. **cpal**: Native low-latency device output and input capture.
2. **ffplay**: Pipes raw `f32` PCM to `ffplay` when compiled without native device headers.
3. **silent**: Runs transport and meters without hardware output when no audio device is reachable.
4. **stream**: Streams raw PCM over a loopback TCP port for remote playback over SSH when `CASSETTE_STREAM_PORT` is set (see [`docs/music-over-ssh.md`](./docs/music-over-ssh.md)).

## Project layout

```text
cassette/
├── src/              Rust audio engine, DSP effects, models, and state reducer
│   ├── app/          Event reducer, undo history, sample loader, spectrum analysis
│   ├── core/         Real-time synth, DSP effects, sample pool, cpal/stream backends
│   ├── events/       Typed engine events and bounded event bus
│   ├── models/       Song, Track, Block, Note, Instrument, and project serialization
│   └── ui/           Serializable UiState snapshot
├── tests/            Integration and real-time zero-allocation tests
└── app/              Desktop application
    ├── src/          Svelte 5 + SvelteKit frontend
    └── src-tauri/    Tauri shell and IPC command handlers
```
