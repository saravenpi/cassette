# Music over SSH

How to hear zboub69 when the machine running the engine is not the machine you
are sitting at.

**Current state.** The streaming backend still exists in the engine, still
works, and is still covered by a test, but no front end can start it. The
desktop app does not surface it, and the terminal UI and its `zboub69
--stream-port` flag were removed. Section 2 says what you would have to do to
use it. The protocol facts in sections 3, 5 and 6 and the local player recipe
are still correct, which is why this file is worth keeping.

---

## 1. Why the server is silent

When you run the engine on a box you are not sitting at, the audio is rendered
*there*. Three things get in the way:

1. **The speakers are on the wrong machine.** Even when the server has working
   audio, it comes out of the server's speakers, which you are not near.
2. **Headless servers often have no usable output.** A typical box only exposes
   HDMI audio devices (`pcmC0D3p`, `pcmC0D7p`, …). ALSA's `default` PCM points at
   an analog device that may not exist, so playback fails with
   `dmix: unable to open slave` or `Unknown PCM default`.
3. **Your user may not be able to open the device at all.** The `/dev/snd` nodes
   are owned by `root:audio`, so a user outside the `audio` group gets
   `cannot find card '0'`.

The stream backend sidesteps all three: it renders on the server, writes raw PCM
to a loopback socket, and you forward that socket over SSH and play it on your
own sound card. The server needs no sound card at all.

---

## 2. The stream backend today

### What still works

`TcpStreamBackend` lives in `src/core/audio.rs`. It binds `127.0.0.1`, waits for
a client, then renders and writes paced PCM to that client. `AudioEngine::
start_streaming(song, port)` selects it through `BackendPreference::Stream`.

The integration test `streaming_only_renders_once_a_client_connects` in
`tests/test_engine.rs` drives it end to end: it asserts the transport stays at
frame 0 while nobody is connected, then connects a `TcpStream` and asserts the
position advances without running ahead of real time. It runs with the rest of
the suite:

```bash
cargo test --test test_engine
```

### What no longer works

[SUPERSEDED by: removal of the terminal UI, 2026-09-24] These instructions
started the backend from a terminal binary:

```bash
zboub69 --stream-port 9999
```

[SUPERSEDED by: removal of the terminal UI, 2026-09-24] There is no `zboub69`
binary any more, and no `--stream-port` flag or `ZBOUB_STREAM_PORT` variable is
read anywhere in the source. The status-bar line, the `space` transport key, the
`?` help key and the port shown in the interface all went with the TUI.

The module doc comment on `TcpStreamBackend` in `src/core/audio.rs` still shows
the removed command; treat it as stale.

### What you would have to do

The backend is reachable only through the engine API:

```rust
use zboub69::{AudioEngine, Song};

let song = Song::new();
let engine = AudioEngine::start_streaming(&song, 9999)?;
let port = engine.stream_port().expect("the stream backend reports its port");
```

Pass `0` for the port and the OS picks a free one; `stream_port()` then returns
the port actually bound. `App::new_streaming(port)` wraps the same call at the
application level. Those two functions are the whole public surface:
`AudioEngine::start_streaming` is called from `src/app.rs`, from the engine
module's own tests, and from `tests/test_engine.rs`, and nowhere else.

To use the backend from the desktop app you would have to start it there.
`start_app()` in `app/src-tauri/src/lib.rs` calls `App::new()` and falls back to
`App::new_silent()`; nothing calls `App::new_streaming`. Note also that the app
header renders only `backend` and `sampleRate`. The `backendNote` field is in
the snapshot and in `app/src/lib/types.ts`, but no component displays it, and
that note is where the port appears, so you would want to render it to find out
which port to forward.

### On your local machine

This half is unchanged and still correct. Open the tunnel in one terminal and
keep it running:

```bash
ssh -N -L 9999:127.0.0.1:9999 yann@your-server
```

Then play the stream in another:

```bash
ffplay -hide_banner -nodisp -autoexit \
  -f f32le -ar 44100 -ch_layout stereo tcp://127.0.0.1:9999
```

`ffplay` runs locally, so the sound comes out of your speakers. Nothing is
rendered until a player connects, so the transport will not run away while
nobody is listening.

### Ports

[SUPERSEDED by: removal of the terminal UI, 2026-09-24] The flag and the
environment variable that used to choose the port:

```bash
zboub69 --stream-port 0
# status bar: streaming PCM on 127.0.0.1:47123

ZBOUB_STREAM_PORT=9999 zboub69
```

The behaviour survives in the API. Pass `0` to `start_streaming` or
`new_streaming` to let the OS pick, then read the port back:

```rust
let engine = AudioEngine::start_streaming(&song, 0)?;
let port = engine.stream_port().expect("a port");
```

Forward whichever port you get. The socket only ever binds `127.0.0.1`
(`TcpStreamBackend::new` hardcodes it), so it is not exposed on the network. The
SSH tunnel is what carries it to you.

---

## 3. Latency

Playback arrives late by however much your link, the tunnel and the player's
buffer add up to. That total is not measured anywhere in this repository, so
treat any figure you see as a rule of thumb rather than a number the engine
guarantees. To tighten it:

- Use a wired connection; WiFi jitter shows up directly.
- Run `ffplay` with a smaller audio buffer if your sound card tolerates it.
- Avoid layering PulseAudio's own buffering on top of the player.
- The engine passes `-fflags nobuffer` to its own `ffplay` invocation
  (`player_args` in `src/core/audio.rs`). Whether adding it to a manual command
  buys you anything is unverified.

The stream itself is paced against the wall clock, so latency does not grow over
time: if your player falls behind, blocking writes push back on the render loop
instead of accumulating an ever-growing backlog.

---

## 4. Troubleshooting

**`Option not found` for `-ac` / `-ch_layout`**

Your local ffmpeg version spells it differently:

| ffmpeg | flag |
|--------|------|
| 6 or newer | `-ch_layout stereo` |
| 5 or older | `-ac 2` |

Check with `ffplay -version`.

**`Connection refused` on the local ffplay**

The tunnel is not up, or the ports do not match. Confirm the `ssh -N -L` command
is still running in its terminal, and that the local port in `-L` matches the
one `stream_port()` reported (or the one in the engine's note, if you rendered
it).

**`Address already in use` on the server**

[SUPERSEDED by: removal of the terminal UI, 2026-09-24] This entry used to say
to start zboub69 with `--stream-port 0`. There is no flag: pass `0` to
`start_streaming` instead. Note that `select_backend` catches the bind failure
and falls back to the null backend with the note `silent: cannot bind
127.0.0.1:<port>: …`, so a taken port shows up as a silent engine rather than as
an error, and `stream_port()` returns `None`.

**Nothing happens when I press space**

[SUPERSEDED by: removal of the terminal UI, 2026-09-24] There is no `space` key
any more; the engine-level equivalent is `RtCommand::Play`. With the stream
backend, no client means no rendering: the transport stays at frame 0 until a
player connects. Start the local `ffplay` first.

**Sometimes it starts mid-transport**

The transport renders only while a client is attached, but it keeps its position
when one disconnects, so a reconnecting player resumes where the last one
stopped. `RtCommand::Stop` rewinds to frame 0, so stopping and playing again
gives you a clean start.

**I would rather fix the server's own audio than stream**

Two options:

```bash
# 1. Make /dev/snd readable (then log out and back in)
sudo usermod -aG audio "$USER"

# 2. Route ALSA's default somewhere real, e.g. the first HDMI output
cat > ~/.asoundrc <<'EOF'
pcm.!default {
    type hw
    card 0
    device 3
}
ctl.!default {
    type hw
    card 0
}
EOF
```

For the best latency on a machine you control, install the ALSA headers and use
the `cpal` backend, which enumerates every output instead of guessing:

```bash
sudo apt install libasound2-dev
```

[SUPERSEDED by: removal of the terminal UI, 2026-09-24] This section used to end
with `cargo run --features cpal`, which needed the terminal binary. The crate
now exposes only a library, so build the desktop shell instead. It already
enables the engine's `cpal` feature (`app/src-tauri/Cargo.toml`), and you build
it from `app/` with `bun run tauri`.

Note that fixing the server's audio still plays it out of the server's speakers.
Streaming is what gets it to you.

---

## 5. If you cannot stream live

Capture the stream to a file locally instead of playing it as it arrives, then
listen whenever you like:

```bash
# terminal 1 — keep the tunnel open
ssh -N -L 9999:127.0.0.1:9999 yann@your-server

# terminal 2 — record the raw PCM the server renders
ffmpeg -f f32le -ar 44100 -ch_layout stereo \
  -i tcp://127.0.0.1:9999 zboub69-mix.wav
```

[SUPERSEDED by: removal of the terminal UI, 2026-09-24] This used to say to
press `space` in the TUI once `ffmpeg` has attached. Start the transport at the
engine level with `RtCommand::Play` instead, once `ffmpeg` is connected; before
that there is nothing to record. ffmpeg writes a normal 16-bit PCM WAV, so you
can play it back anywhere afterwards. This still needs the tunnel and a local
ffmpeg, but no sound card on either end.

---

## 6. How the stream backend behaves

Worth knowing when something looks stuck:

- **Format:** raw 32-bit little-endian stereo PCM, 44.1 kHz, no header. Each
  `f32` goes out as `to_le_bytes()`.
- **One client at a time.** When it disconnects, the backend waits for the next
  connection; it does not fall back to another device.
- **No client, no rendering.** The transport stays put rather than advancing
  silently.
- **Wall-clock paced.** A client that drains the socket instantly (for example
  `cat tcp://` or `nc`) cannot drag the transport faster than real time; the
  writer sleeps on a deadline.
- **Backpressure, not unbounded buffering.** If the client is slower than real
  time, the write blocks and the transport waits for it.
- **Reported by the engine.** `AudioEngine::backend_name()`, `stream_port()` and
  `note()` describe the running backend. [SUPERSEDED by: removal of the terminal
  UI, 2026-09-24] The status bar used to print them and `?` opened the in-app
  help; neither exists now. The desktop app shows the backend name and sample
  rate only, so the port is not visible anywhere in the interface today.
