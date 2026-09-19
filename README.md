# zboub69 — a terminal DAW

A keyboard-only digital audio workstation that runs in your terminal, inspired
by Ableton Live's session view. Build a pattern on a step grid, play it with a
built-in polyphonic synth, and bounce it to a WAV file — all without leaving
the shell.

```
┌ Transport ────────────────────────────────────────────────┐
│ ▶ playing   bpm 110   step 3/16   loop on   silent @ 44100│
├ Mixer ──────────┬ Pattern ──────────────┬ Instrument ─────┤
│  1 Kick   --####│     1   2   3   4     │ Kick            │
│  2 Snare  --#---│ Kick ● · · · ● · · ·  │ preset   Kick   │
│  3 Hat    ----- │ Snare · · · ● · · · · │ waveform Sine   │
│  4 Bass   --##--│ Hat  · · ● · · · ● ·  │ decay    0.280s │
└─────────────────┴───────────────────────┴─────────────────┘
 space play · enter step · arrows move · [ ] pitch · M mute · ? help
```

## Run it

```bash
cargo run                       # full-screen TUI
cargo run -- --stream-port 9999 # TUI + audio streamed over loopback TCP (SSH)
cargo run -- --headless         # print one plain-text frame and exit
cargo test                      # 97 tests
```

## Controls

| Key | Action |
|-----|--------|
| `space` | play / stop |
| `enter` | toggle the step under the cursor |
| arrow keys | move the pattern cursor (rows are tracks) |
| `[` `]` | transpose the step |
| `-` `=` | step velocity |
| `,` `.` | track volume |
| `P` `N` | pan left / right |
| `M` `S` | mute / solo the selected track |
| `W` `I` | cycle waveform / instrument preset |
| `+` `-` | pattern length |
| `<` `>` | tempo |
| `tab` | cycle pane focus |
| `R` | arm performance recording |
| `L` | toggle looping |
| `E` | bounce to `zboub69-mix.wav` |
| `z s x d c v g b h n j m` | play the lower octave |
| `q 2 w 3 e r 5 t 6 y 7 u` | play the upper octave |
| `pgup` / `pgdn` | shift the play octave |
| `?` | help overlay |
| `Q` / `ctrl-c` | quit |

Lower-case letters are the piano; upper-case letters and punctuation are
commands, so playing and editing never collide. Press `R`, then play: your
notes are quantised into the pattern as you go.

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

You do not need audio to work on music: `E` bounces the pattern to a 16-bit WAV
at any time, rendered by the exact same engine.

### Audio over SSH

When you are connected over SSH, the machine running zboub69 is not the machine
with the speakers — and a headless server often has no usable output at all.
Stream the audio down the connection and play it locally instead.

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

Now press `space` in the TUI. `ffplay` consumes the stream in real time, which
is what paces the transport, so nothing is rendered until a player connects.
Expect a few hundred milliseconds of latency — fine for composing, not for
monitoring a live input. On ffmpeg 5 or older, replace `-ch_layout stereo` with
`-ac 2`.

The stream is raw 32-bit little-endian stereo PCM at 44.1 kHz. `--stream-port 0`
asks the OS to choose a free port, which is reported in the status bar.

If you would rather not set up a tunnel, `E` bounces the pattern to a WAV:
`scp server:zboub69-mix.wav .` and play it locally.

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
│   ├── sequencer.rs   sample-accurate step clock
│   ├── render.rs      RenderCore + the RtCommand vocabulary
│   ├── audio.rs       backends (cpal / ffplay / silent) + SharedState
│   ├── engine.rs      control side: command producer + shared state
│   ├── offline.rs     WAV bouncing (Phase 4.1)
│   └── recorder.rs    meters + performance recording (Phase 4.2)
├── events/            bounded fan-out EventBus + typed Event
├── input/             keymap, terminal handler, MIDI/OSC adapters (Phase 4.3)
├── models/            Song, Track, Step, Instrument — plain data
├── ui/                three-pane renderer, UiState + View (Phase 4.4)
└── utils/             pitch, tempo and math helpers
```

### Real-time contract

`core::render::RenderCore::process` runs inside the audio callback. It never
allocates, locks, performs I/O or panics: voices and step grids are fixed-size
arrays, commands cross threads through an `rtrb` SPSC ring buffer, and state
comes back through atomics. The offline renderer runs the same DSP loop under
`assert_no_alloc`, and a test asserts the bounce allocates nothing.

See `docs/ARCHITECTURE.md` for the full layer rules and `ROADMAP.md` for status.

## Testing

95 tests: unit tests for every module plus integration tests for the engine,
the offline renderer and the input adapters.

```bash
cargo test
cargo clippy --all-targets   # clean
```

Integration tests cover the engine's command path, the TCP stream backend
(including that it does not render without a client), the WAV bounce and the
MIDI/OSC adapters.
