# zboub69 — a terminal DAW

A keyboard-only digital audio workstation that runs in your terminal, inspired
by Ableton Live's session view. Lay out MIDI and audio tracks, add blocks to
them, and play the built-in polyphonic synth from your keyboard — all without
leaving the shell.

```
┌ Transport ────────────────────────────────────────────────┐
│ ▶ playing   bpm 110   bar 3.2   loop on   silent @ 44100  │
├ Tracks ───────────────────────────┬ Inspector ────────────┤
│  ●  1 Midi 1   MIDI  Verse Chorus │ Midi 1                │
│  ●  2 Midi 2   MIDI  +            │ kind     MIDI         │
│  ·  3 Audio 1  AUDIO +            │ blocks   2            │
│  ·  4 Audio 2  AUDIO +            │ volume   0.80         │
└───────────────────────────────────┴───────────────────────┘
 space play · arrows move · A add block · D delete · enter rename · ? help
```

Blocks are empty slots for now: notes for MIDI tracks and audio files for audio
tracks come later.

## Run it

```bash
cargo run                       # full-screen TUI
cargo run -- --stream-port 9999 # TUI + audio streamed over loopback TCP (SSH)
cargo run -- --headless         # print one plain-text frame and exit
cargo test                      # 93 tests
```

## Controls

| Key | Action |
|-----|--------|
| `space` | play / stop |
| `L` | toggle looping |
| `up` `down` | move between tracks |
| `left` `right` | move between blocks on the selected track |
| `A` | add a block to the selected track |
| `D` | delete the block under the cursor |
| `K` | flip the track between MIDI and audio |
| `enter` | rename the block under the cursor |
| `esc` | cancel the rename |
| `,` `.` | track volume |
| `P` `N` | pan left / right |
| `M` `S` | mute / solo the selected track |
| `<` `>` | tempo |
| `z s x d c v g b h n j m` | play the lower octave |
| `q 2 w 3 e r 5 t 6 y 7 u` | play the upper octave |
| `pgup` / `pgdn` | shift the play octave |
| `?` | help overlay |
| `Q` / `ctrl-c` | quit |

Lower-case letters are the piano; upper-case letters and punctuation are
commands, so playing and editing never collide. Press `A` to add a block, then
`enter` to name it.

## Audio backends

The engine picks the best sink it can and tells you which one it got in the
status bar:

1. **cpal** — direct device output, lowest latency. Enable it with the `cpal`
   feature (see below).
2. **ffplay** — streams raw `f32` PCM into ffmpeg's player. Needs no development
   headers, but the player must be able to open your audio device.
3. **silent** — the transport still runs and the meters still move; nothing
   comes out. Used when no device is reachable.

### Getting sound

If the status bar says `silent`, the note after it says why. The usual causes:

* **No permission on `/dev/snd`.** Add yourself to the `audio` group, then log
  out and back in:
  ```bash
  sudo usermod -aG audio "$USER"
  ```
* **The ALSA `default` device points at hardware you do not have** (for example
  a machine with only HDMI outputs). Route it somewhere real with `~/.asoundrc`,
  or use cpal, which enumerates every output.
* **Low latency.** Install the ALSA headers and build with the `cpal` feature:
  ```bash
  sudo apt install libasound2-dev
  cargo run --features cpal
  ```

You do not need a working audio device to compose: the transport, the meters and
the keyboard all behave the same when the engine falls back to `silent`.

### Audio over SSH

When you are connected over SSH, the machine running zboub69 is not the machine
with the speakers — and a headless server often has no usable output at all.
Stream the audio down the connection and play it locally instead.

> Full guide, including troubleshooting and alternatives:
> [`docs/music-over-ssh.md`](./docs/music-over-ssh.md).

On the server:

```bash
zboub69 --stream-port 9999
```

The status bar will say it is streaming on `127.0.0.1:9999`. On **your own**
machine, forward that port and play the stream:

```bash
# terminal 1 — keep the tunnel open
ssh -N -L 9999:127.0.0.1:9999 user@server

# terminal 2 — play what the remote DAW renders
ffplay -hide_banner -nodisp -autoexit \
  -f f32le -ar 44100 -ch_layout stereo tcp://127.0.0.1:9999
```

Now press `space` in the TUI. Nothing is rendered until a player connects, and
the stream is paced against the wall clock, so a client that drains the socket
instantly cannot drag the transport along. Expect a few hundred milliseconds of
latency — fine for composing, not for monitoring a live input. On ffmpeg 5 or
older, replace `-ch_layout stereo` with `-ac 2`.

The stream is raw 32-bit little-endian stereo PCM at 44.1 kHz. `--stream-port 0`
asks the OS to choose a free port, which is reported in the status bar.

If you would rather not set up a tunnel, you can capture the stream on your local
machine instead of playing it live:

```bash
ssh -N -L 9999:127.0.0.1:9999 user@server
ffmpeg -f f32le -ar 44100 -ch_layout stereo -i tcp://127.0.0.1:9999 mix.wav
```

## Architecture

Dependencies point inward, and the audio path never touches the terminal:

```
input ──► events ──► core ◄── ui
                      ▲
                    models   (shared data only)
```

```
src/
├── main.rs            entry point (TUI or --headless)
├── lib.rs             public API (crate name: terminal_daw)
├── app.rs             orchestrator: terminal, event loop, command handling
├── core/              audio engine — no terminal, no models mutation by UI
│   ├── synth.rs       polyphonic voices + ADSR, allocation-free
│   ├── render.rs      RenderCore + Transport + the RtCommand vocabulary
│   ├── audio.rs       backends (cpal / ffplay / silent / stream) + SharedState
│   ├── engine.rs      control side: command producer + shared state
│   └── meter.rs       level meters (Phase 4.2)
├── events/            bounded fan-out EventBus + typed Event
├── input/             keymap, terminal handler, MIDI/OSC adapters (Phase 4.3)
├── models/            Song, Track, Block, Instrument — plain data
├── ui/                session track list + inspector, UiState + View (Phase 4.4)
└── utils/             pitch, tempo and math helpers
```

### Real-time contract

`core::render::RenderCore::process` runs inside the audio callback. It never
allocates, locks, performs I/O or panics: voices, the block-name mirror and the
transport are fixed-size, commands cross threads through an `rtrb` SPSC ring
buffer, and state comes back through atomics. `tests/test_rt_no_alloc.rs` runs the
render loop and the command path under `assert_no_alloc`, so the claim fails the
build rather than the audio thread if it ever stops being true.

See `docs/ARCHITECTURE.md` for the full layer rules and `ROADMAP.md` for status.

## Testing

93 tests: unit tests for every module plus integration tests for the engine and
the input adapters.

```bash
cargo test
cargo clippy --all-targets   # clean
```

Integration tests cover the engine's command path, the TCP stream backend
(including that it does not render without a client) and the MIDI/OSC adapters.
