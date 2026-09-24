# Music over SSH

How to hear zboub69 when the machine running it is not the machine you are
sitting at.

---

## 1. Why the server is silent

When you SSH into a box and start the DAW, the audio is rendered *there*. Three
things get in the way:

1. **The speakers are on the wrong machine.** Even when the server has working
   audio, it comes out of the server's speakers, which you are not near.
2. **Headless servers often have no usable output.** A typical box only exposes
   HDMI audio devices (`pcmC0D3p`, `pcmC0D7p`, …). ALSA's `default` PCM points at
   an analog device that may not exist, so playback fails with
   `dmix: unable to open slave` or `Unknown PCM default`.
3. **Your user may not be able to open the device at all.** The `/dev/snd` nodes
   are owned by `root:audio`, so a user outside the `audio` group gets
   `cannot find card '0'`.

The stream backend sidesteps all three: it renders on the server, sends raw PCM
down the SSH connection, and plays it on your machine through your own sound
card. The server needs no sound card at all.

---

## 2. Stream it over SSH (recommended)

### On the server

```bash
zboub69 --stream-port 9999
```

The status bar will say:

```
audio 'stream' at 44100 Hz — streaming PCM on 127.0.0.1:9999
```

Nothing is rendered until a player connects, so the transport will not run away
while nobody is listening.

### On your local machine

Open the tunnel in one terminal and keep it running:

```bash
ssh -N -L 9999:127.0.0.1:9999 yann@your-server
```

Then play the stream in another:

```bash
ffplay -hide_banner -nodisp -autoexit \
  -f f32le -ar 44100 -ch_layout stereo tcp://127.0.0.1:9999
```

Now press `space` in the TUI. `ffplay` is running locally, so the sound comes
out of your speakers.

### Ports

`--stream-port 0` asks the OS for a free port and reports the real one in the
status bar — useful when 9999 is taken:

```bash
zboub69 --stream-port 0
# status bar: streaming PCM on 127.0.0.1:47123
```

Forward whichever port it reports. The environment variable
`ZBOUB_STREAM_PORT` works too, for when you would rather not type a flag:

```bash
ZBOUB_STREAM_PORT=9999 zboub69
```

The socket only ever binds `127.0.0.1`, so it is not exposed on the network. The
SSH tunnel is what carries it to you.

---

## 3. Latency

Playback has a few hundred milliseconds of delay — fine for composing, not for
monitoring a live input. To tighten it:

- Use a wired connection; WiFi jitter shows up directly.
- Run `ffplay` with a smaller audio buffer if your sound card tolerates it.
- Locally, prefer `-fflags nobuffer` (already implied by the raw stream) and
  avoid layering PulseAudio's own buffering on top.

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
is still running in its terminal, and that the local port in `-L` matches what
the status bar reported.

**`Address already in use` on the server**

Something already holds that port. Start zboub69 with `--stream-port 0` and use
the port it reports.

**Nothing happens when I press space**

Expected until a player connects. Start the local `ffplay` first; the transport
begins as soon as it attaches.

**Sometimes it starts mid-transport**

The transport only starts when a client connects, but it also keeps running
while one is attached. If you want a clean start, press `space` twice (stop then
play) after connecting.

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
cargo run --features cpal
```

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

Press `space` in the TUI once `ffmpeg` has attached. ffmpeg writes a normal
16-bit PCM WAV, so you can play it back anywhere afterwards. This still needs
the tunnel and a local ffmpeg, but no sound card on either end.

---

## 6. How the stream backend behaves

Worth knowing when something looks stuck:

- **Format:** raw 32-bit little-endian stereo PCM, 44.1 kHz, no header.
- **One client at a time.** When it disconnects, the backend waits for the next
  connection; it does not fall back to another device.
- **No client, no rendering.** The transport stays put rather than advancing
  silently.
- **Wall-clock paced.** A client that drains the socket instantly (for example
  `cat tcp://` or `nc`) cannot drag the transport faster than real time.
- **Backpressure, not unbounded buffering.** If the client is slower than real
  time, the write blocks and the transport waits for it.
- **Reported in the UI.** The status bar names the backend and the port; `?`
  opens the in-app help.
