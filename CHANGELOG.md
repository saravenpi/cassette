# Changelog

All notable changes to zboub69 are recorded here.

## [0.1.0] - 2026-09-24

The first release: a keyboard-only terminal DAW.

### Added

- Session view: a track list of MIDI and audio tracks, each with named blocks, a
  `+` slot, a level meter and a volume, plus an inspector for the selected track.
- Block editing: `A` adds a block, `D` deletes the block under the cursor, `enter`
  renames it, and left/right move between blocks.
- Transport: play/stop, tempo, looping and a bar/beat readout.
- A polyphonic synth with waveform and ADSR instrument presets, played from the
  keyboard over two octaves, with the keyboard octave shifted by page up/down.
- Mixer: volume, pan, mute, solo, and smoothed level meters.
- Audio backends: cpal, an ffplay subprocess, loopback TCP for SSH sessions, and
  a silent backend for headless runs.
- Input adapters for MIDI and OSC.
- A plain-text renderer: `zboub69 --headless`.
- A debug-only test that runs the audio path under `assert_no_alloc`.

### Removed

- The step sequencer, the step grid and the pattern model, replaced by blocks.
- Offline rendering and the WAV bounce (`E`).
- Performance recording of live notes.

## [0.2.0] - 2026-09-24

The interface moves out of the terminal. The audio engine is unchanged in kind and
still owns all timing and synthesis.

### Added

- A desktop app: a SvelteKit frontend served by a Tauri shell, with the engine
  running on a thread of its own. Keys and clicks both drive it.
- `ZBOUB_STREAM_PORT`, a front door for the audio-over-SSH streaming backend.
- Keymap unit tests: `cd app && bun test`.
- A full per-platform icon set, and explicit bundle targets (`deb`, `appimage`,
  `nsis`, `dmg`) in place of `all`.
- The app header renders `backendNote`, so the stream port is visible.

### Changed

- The crate is renamed `terminal_daw` to `zboub69`, and is now library-only.
- `ui::view::UiState` is the wire contract and derives serde; `events::Event` is
  the intent vocabulary the webview sends back.

### Fixed

- The `cpal` feature did not compile (`render` was an immutable binding). The
  backend had been dead code behind a non-default feature. Found by building the
  shell, which enables the feature.
- Space could fire a focused transport button, because only the keydown default
  was cancelled. A button runs its Space activation on keyup.
- The README listed `S` for solo; the keymap binds `O`, because `S` is a piano note.
- Keyup read the note pitch without consulting rename mode.
- `cargo doc` warned on a private intra-doc link.

### Removed

- The terminal interface: `ratatui`, `crossterm`, `src/main.rs`, the terminal
  keymap and input handler, the terminal lifecycle, and the `--headless` and
  `--stream-port` flags.
