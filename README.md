# TerminalDAW

Ableton-like terminal DAW, 100% keyboard driven. Built with [ratatui](https://github.com/ratatui/ratatui) and [crossterm](https://github.com/crossterm/crossterm).

## Status

This is an empty skeleton. **No DAW features are implemented yet.** The app currently renders a static three-pane layout and exits on `q`. All domain logic (audio engine, MIDI, tracks, clips, mixer) is pending.

**Research completed 2026-09-16:** See `docs/rust-daw-research.md` for architectural patterns from `naiqvist/daw`, `sourdaw`, `generic-daw`, `Meadowlark`, and a Tauri+cpal teaching project.

### Key findings applied to this project

1. **EventBus is currently unbounded** — `src/events/bus.rs` uses `std::sync::mpsc::channel()` (unbounded). Per research, should be `crossbeam::bounded` or `rtrb` to keep tail latency predictable.
2. **Audio thread boundary is undefined** — no cpal/rtaudio integration yet. When added, the audio callback closure must `move`-own its Mixer (no `Arc<Mutex<Mixer>>`). See `docs/rust-daw-research.md` §2.4.
3. **Pre-allocation needed** — `core/mixer.rs` is empty. Follow the `[Option<AudioNode>; 32]` pre-allocation pattern when implementing.
4. **Dependency direction is correct** — `input → events → core ← ui` with `models` shared. Preserve this.

## Architecture

The project follows a clean architecture with the layers defined under `terminal_daw/src/`. All modules communicate through `EventBus`. The engine processes events and mutates state; the renderer reads state and draws to the terminal. No circular dependencies.

```
┌──────────────────────────────────────────────┐
│  src/main.rs                                 │
│  Entry point: boots the App, runs the loop   │
├──────────────────────────────────────────────┤
│  src/app.rs                                  │
│  Orchestrator wiring all layers together     │
├──────────┬──────────┬──────────┬─────────────┤
│ src/ui/  │ events/  │ input/   │  src/core/  │
│ Rendering│ EventBus │ Keyboard │ Audio/MIDI/ │
│ & layout │          │ mapping  │ Tracks/etc  │
├──────────┴──────────┴──────────┴─────────────┤
│  src/models/      │  src/utils/             │
│  Domain data      │  Audio math, tempo,     │
│  (tracks, clips,  │  frequency helpers      │
│   patterns, …)    │                         │
└──────────────────────────────────────────────┘
```

### Layers

| Layer | Path | Responsibility |
|---|---|---|
| **Entry** | `src/main.rs` | Bootstrap terminal, run the draw/input loop |
| **App (orchestrator)** | `src/app.rs` | Wire all layers, hold top-level state |
| **UI** | `src/ui/` | Terminal rendering, layout, views. Depends on core types only |
| **Events** | `src/events/` | Event bus decoupling UI from engine |
| **Input** | `src/input/` | Keyboard mapping and input handling |
| **Core** | `src/core/` | Audio engine, MIDI, tracks, clips, mixer (no UI dependency) |
| **Models** | `src/models/` | Domain data models (tracks, clips, patterns, instruments) |
| **Utils** | `src/utils/` | Audio math, tempo, frequency helpers |

## Run

```sh
cargo run
```

Quit with `q`.

## Dependencies

| Crate | Purpose |
|---|---|
| `ratatui` | Terminal UI rendering |
| `crossterm` | Terminal manipulation and input events |
| `anyhow` | Error handling |
