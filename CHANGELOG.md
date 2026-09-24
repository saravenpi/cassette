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