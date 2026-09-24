# zboub69

A session DAW inspired by Ableton Live's session view. Lay out MIDI and audio
tracks, add blocks to them, and play the built-in polyphonic synth from your
keyboard.

The audio engine is Rust and does all the timing and synthesis. The interface is
SvelteKit, served by a thin Tauri shell. The webview only draws: it sends intent
and reads a snapshot the engine publishes once a frame.

```
zboub69/
├── src/              the engine and the app logic (crate: zboub69)
├── tests/            engine integration tests
├── docs/             architecture notes
└── app/              the desktop app
    ├── src/          SvelteKit frontend (routes, bridge, keymap)
    └── src-tauri/    the Tauri shell: two commands over an engine thread
```

Blocks are empty slots for now: notes for MIDI tracks and audio files for audio
tracks come later.

## Run it

You need Rust, [bun](https://bun.sh), and Tauri's Linux system dependencies:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libdbus-1-dev
```

Then:

```bash
cd app
bun install
bun run tauri dev      # dev window, frontend hot-reloads
bun run tauri build    # bundle for this platform
```

The engine on its own needs neither a webview nor those system libraries:

```bash
cargo test             # 81 tests
cargo clippy --all-targets
```

## Controls

| Key | Action |
|-----|--------|
| `space` | play / stop |
| `L` | toggle looping |
| `up` `down` | move between tracks |
| `left` `right` | move between blocks on the selected track |
| `N` / `enter` | add a block to the selected track |
| `backspace` / `del` | delete the block under the cursor |
| `C` | flip the track between MIDI and audio |
| `R` | rename the block under the cursor |
| `esc` | cancel the rename |
| `-` `=` | track volume |
| `[` `]` | pan left / right |
| `,` `.` | tempo |
| `M` `S` | mute / solo the selected track |
| `Z` `X` | shift the play octave |
| `A W S E D F T G Y H U J K` | play notes |

Commands sit on keys the piano row does not use, so playing and editing never
collide: notes are checked first, and a key bound to both would make the command
silently unreachable. `app/src/lib/keys.ts` refuses to load if that ever happens.

Everything is also clickable: the transport, the block slots, mute and solo.

## Audio backends

The engine picks the best sink it can and names it in the status bar:

1. **cpal** — direct device output, lowest latency. The desktop app builds with
   the engine's `cpal` feature on.
2. **ffplay** — streams raw `f32` PCM into ffmpeg's player. Needs no development
   headers, but the player must be able to open your audio device.
3. **silent** — the transport still runs and the meters still move; nothing comes
   out. The app falls back to this if no device is reachable, so the window
   always opens.

### Getting sound

If the status bar says `silent`, the note after it says why. The usual causes:

* **No permission on `/dev/snd`.** Add yourself to the `audio` group, then log
  out and back in:
  ```bash
  sudo usermod -aG audio "$USER"
  ```
* **The ALSA `default` device points at hardware you do not have** (for example
  a machine with only HDMI outputs). Route it somewhere real with `~/.asoundrc`,
  or build with the `cpal` feature, which enumerates every output.

You do not need a working audio device to compose: the transport, the meters and
the keyboard all behave the same on the silent backend.

### Audio over SSH

The streaming backend is still in the engine and still tested
(`AudioEngine::start_streaming`, `App::new_streaming`), but no front-end exposes
it: the terminal build's `--stream-port` flag went with the TUI. The protocol
notes and the ffplay side of it are in [`docs/music-over-ssh.md`](./docs/music-over-ssh.md).

## Architecture

Dependencies point inward, and the audio path never touches the interface:

```
input ──► events ──► core ◄── ui
                       ▲
                     models   (shared data only)
```

```
src/
├── lib.rs             public API (crate name: zboub69)
├── app.rs             intent handling + the UiState snapshot
├── core/              audio engine — no frontend, no terminal
│   ├── synth.rs       polyphonic voices + ADSR, allocation-free
│   ├── render.rs      RenderCore + Transport + the RtCommand vocabulary
│   ├── audio.rs       backends (cpal / ffplay / silent / stream) + SharedState
│   ├── engine.rs      control side: command producer + shared state
│   └── meter.rs       level meters
├── events/            bounded fan-out EventBus + typed Event
├── input/             MIDI/OSC adapters
├── models/            Song, Track, Block, Instrument — plain data
├── ui/                UiState — the serializable snapshot the webview draws
└── utils/             pitch, tempo and math helpers

app/src-tauri/src/
├── lib.rs             the two commands (state, send) + the engine thread
└── main.rs            desktop entry point
```

### Why the engine has its own thread

The engine owns a cpal `Stream`, and cpal marks `Stream` `!Send` and `!Sync` on
every platform. It therefore cannot live in Tauri's managed state, which must be
`Send + Sync`. So the shell gives it a thread of its own: the webview sends
intent down a channel, and the engine thread applies it and publishes a snapshot.

A command wakes that thread at once instead of on the next frame, so a note
sounds the moment the key goes down. Nothing about timing or synthesis is
decided in JavaScript, and the webview cannot draw faster than 60 Hz anyway.

### Real-time contract

`core::render::RenderCore::process` runs inside the audio callback. It never
allocates, locks, performs I/O or panics: voices, the block-name mirror and the
transport are fixed-size, commands cross threads through an `rtrb` SPSC ring
buffer, and state comes back through atomics. `tests/test_rt_no_alloc.rs` runs the
render loop and the command path under `assert_no_alloc`, so the claim fails the
build rather than the audio thread if it ever stops being true.

See `docs/ARCHITECTURE.md` for the full layer rules and `ROADMAP.md` for status.

## Testing

81 tests: unit tests for every engine module plus integration tests for the
engine, the TCP stream backend and the MIDI/OSC adapters.

```bash
cargo test
cargo clippy --all-targets   # clean
```

The frontend type-checks separately:

```bash
cd app && bun run check
```