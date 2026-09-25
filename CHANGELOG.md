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

## [0.3.0] - 2026-09-25

The session view becomes a timeline. A block now sits at a position and holds a
length, a track carries MIDI or audio, and an audio sample dropped on a lane plays
when the playhead reaches its block. This release removes public API: see
Removed.

### Added

- The timeline: a bar/beat ruler, one lane per track, blocks placed at an absolute
  beat position with a length in beats, and a playhead.
- A left sidebar with three panes: a file browser over `$ZBOUB_SAMPLES_DIR` (else
  `~/Music`), the built-in voices, and the selected track's voice editor.
- A WAV decoder for PCM 8/16/24/32-bit and IEEE float 32/64-bit, downmixed to
  mono, with a 256 MiB file ceiling.
- Sample playback: a fixed 8-slot pool allocated before the stream starts, fed by
  an `rtrb` ring, with one sample voice per track started by the playhead.
- A block's end cuts its sample with a short fade, so a sample longer than its
  block is not left running past it.
- Block dragging, including across tracks, with grid snapping, `alt` to drag free,
  right-edge resizing, and zoom from 8 to 256 pixels per beat.
- An 8-colour track palette keyed by track index.
- Five Tauri commands for the file browser: `samples_root`, `list_samples`,
  `home_dir`, `parent_of`, `read_waveform`.

### Changed

- `Block` carries `start_beats`, `length_beats` and `source`. `Track` keeps its
  blocks in start order and names new ones from a counter that never repeats, so
  two blocks on a lane cannot share an automatic name.
- The audio thread learns about blocks as `RtCommand::SetBlocks`, one
  `MirroredBlock { start_beats, end_beats, slot, gain }` per block, replacing a
  whole lane at once.
- `UiState` gained `lengthBeats`, `bars`, `beatsPerBar`, `presets` and a per-track
  `gain`; `BlockView` carries its start, length and source.
- `src/app.rs` became `src/app/{mod,timeline,tracks,samples,transport}.rs`.
- The sample ring is sized to hold the largest load whole, because a load sends
  one message per frame.
- The frontend is split into `Sidebar`, `FileBrowser`, `VstPane`, `EffectsPane`,
  `Transport`, `Ruler`, `Timeline`, `TrackHeader`, `TrackLane`, `Block`,
  `Playhead` and `StatusBar`, with `geometry.ts` owning the beat-to-pixel maths.

### Fixed

- A block at the top of the timeline never started. The render core now schedules
  blocks over the buffer it is about to render, so the first block on every lane
  sounds, and a one-shot shorter than a buffer is no longer dropped.
- Every dropped sample was silent: the mirror's gain compared a length in beats
  against a release in seconds, which drove almost every block to zero, and a
  sample voice refuses a gain that is not positive.
- Any sample longer than about 1.5 s was refused, and the status blamed the audio
  thread, because the ring held fewer messages than one load needs.
- A resync rebuilt every lane without its sample slot, silencing audio blocks
  until the next edit. `App::tick` re-pushes each lane after a resync.
- The file browser invoked `parentOf` and `readWaveform` while the shell
  registered `parent_of` and `read_waveform`, so the breadcrumb's up button and
  every waveform were dead.

### Removed

- The session view's list of named block slots.
- `BlockName`, `RtCommand::AddBlock`, `TrackView::add_slot` and
  `app/src/lib/types.ts`, all replaced by the timeline model.

## [0.4.0] - 2026-09-25

Universal audio decoding, arbitrary sample lengths, low-latency playback, timeline seeking, and file browser filtering.

### Added

- Universal audio file decoding via Symphonia: MP3, FLAC, M4A, AAC, OGG, Opus, AIFF, and WAV.
- Native OS drag and drop: dropping audio files from the file manager directly onto timeline lanes.
- Timeline seeking: clicking or dragging on the ruler seeks the transport to that beat.
- `Event::Seek` and `RtCommand::Seek` transport control.
- Non-audio file filtering in the file browser: hidden by default with a count and toggle button.

### Changed

- Sample pool supports arbitrary length audio files through `Arc<[f32]>` buffers without allocations on the audio callback.
- Sample drops within two beats of lane start snap to beat 0.0.

### Fixed

- Audio truncation at 480,000 frames (sample pool) and 200,000 frames (Tauri waveform decoder).
- External file loading: removed sample root directory restriction from waveform reader.
- Playback delay: blocks covering the playhead now start immediately when the track is idle.
- Subprocess latency: added low-delay flags to `ffplay` pipeline and requested 256-frame buffers in `cpal`.

## [0.5.0] - 2026-09-25

Right-aligned track headers, custom UI palette, square geometry, ruler grid alignment, and project audio export and recording.

### Added

- Project audio export: offline render to standard 16-bit stereo WAV with release tail via `ExportAudio` and `encode_pcm16`.
- Take and project recording: direct recording to timestamped WAV files via `RecordAudio` and the transport `Rec` button.
- Project definition serialization: export session structure (tracks, tempo, blocks, presets) to JSON via `ExportProject`.
- Per-track height resizing: drag the bottom border of any track row to adjust its height between 32px and 300px; double-click to reset.
- Transport controls: added `Rec`, `Export WAV`, and `Save` buttons to the header bar.

### Changed

- Background color updated to `#0B0B0B` and primary accent to `#E55B3F`.
- Track palette updated to five distinct tones: `#84B7BD` (blue), `#A8B365` (green), `#D9AF67` (yellow), `#E55B3F` (orange), `#947164` (brown).
- Geometry updated to strict square corners: removed all `border-radius` across the application.
- Track headers (name, type, mute, solo, faders, meter) moved to the right side of lanes, sticky to the right edge.
- Default track height increased from 24px to 64px for clearer waveform rendering and control layout.

### Fixed

- Ruler and track grid misalignment: lanes and ruler now both start at x=0 with identical width and zero playhead offset.


