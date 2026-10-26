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

## [0.6.0] - 2026-09-25

macOS frameless overlay, project loading, flat SVG grid lines, and browser renaming.

### Added

- Project loading: load and restore session structure, BPM, blocks, and sample references from JSON via `LoadProject` and `App::load_project`.
- Project loading in UI: transport `Load` button with file picker, `.json` drag and drop on the window, and double-click to load from the browser.
- Frameless overlay on macOS: `titleBarStyle: Overlay` with `hiddenTitle: true` and macOS-aware top bar padding to integrate window controls cleanly.

### Changed

- Transport export button renamed from "Export WAV" to "Export".
- Sidebar tab renamed from "File browser" to "Browser".
- Timeline lane grid background replaced with flat SVG vector lines, removing all CSS gradients.

## [0.7.0] - 2026-09-26

Renamed project to cassette, updated repository and directory, and created desktop squircle icons.

### Added

- Desktop squircle icon derived from `solar:cassette-2-bold-duotone` on `#0B0B0B` background for macOS (`.icns`), Windows (`.ico`), Linux, iOS, and Android.
- `CASSETTE_SAMPLES_DIR` and `CASSETTE_STREAM_PORT` environment variables with fallback to legacy `ZBOUB_*` variables.
- Support for loading both `cassette-project-*.json` and legacy `zboub69-project-*.json` project files.

### Changed

- Project and repository renamed from zboub69 to cassette (`saravenpi/cassette`).
- Default branch renamed from master to main.
- Rust audio engine crate renamed to `cassette`.
- Tauri desktop shell renamed to `cassette-app`.
- Sample audio file renamed to `cassette-mix.wav`.


## [0.8.0] - 2026-10-26

A review pass over the whole codebase. Audio-thread hazards, export and project loading are fixed; looping and the Load dialog now work.

### Added

- Looping: with loop on, the playhead wraps at the end of the last block. Loop is on by default.
- Project files carry a `version` field and are validated on load: tempo, block start and length, volume, pan and block and track counts are clamped, and blocks are kept sorted.
- A Tauri dialog for the Load button, so the chosen project file is the one loaded.
- Keyboard import and load in the file browser (Enter), and keyboard resize of track rows.
- Multi-file drops are placed end to end, using each block's real length.
- A decode ceiling of 10 minutes, with truncation and early-stop reported in the status line.

### Changed

- Beat position and block scheduling are computed in `f64`, and tempo is no longer rounded to whole frames per beat, so non-integer tempos no longer drift.
- The ffplay and TCP backends write on a separate thread through a bounded queue and drop audio instead of stalling the render thread.
- File browser commands run off the UI thread. Symlinks inside the sample root are followed.
- Project and WAV files are written atomically.
- Seek and stop fade sample voices out instead of cutting them.

### Fixed

- Audio export ignored each sample's rate, so non-44.1 kHz files exported at the wrong speed and pitch.
- `SampleMsg::End` could panic the audio thread on an out-of-range slot.
- The block mirror and sample buffers could be freed on the audio thread.
- Loading a project left the previous project's samples, held notes and rename prompt in place.
- Mix and voice changes to two tracks in one frame dropped the first change.
- The drag-drop listener leaked when the page unmounted early.
- The window title bar could not be dragged on macOS.

### Removed

- Dead helpers used only by tests (`block_at`, `contains`, `is_audible`, `utils::math`).

## [0.9.0] - 2026-10-26

MIDI blocks hold notes, the render core plays them, and a piano roll edits them.

### Added

- Notes in MIDI blocks: pitch, start, length and velocity, up to 512 per block, saved in the project file. v0.8.0 projects still load.
- The render core starts each note as the playhead reaches it and releases it after its length; the audio export renders notes too.
- A piano roll under the timeline, opened by double-clicking a MIDI block or pressing `P`: click adds a note, drag moves or resizes it, the wheel sets velocity, `del` removes it, and the keys on the left audition pitches.
- A note preview inside MIDI blocks on the selected track.

### Changed

- A loop wrap releases every synth voice, so a note crossing the loop end stops there.
- The UI snapshot carries notes only for blocks on the selected track.

### Removed

- `RenderCore::peaks`, which nothing called.

## [0.10.0] - 2026-10-26

Loop points, frame-accurate note starts, and a review pass over v0.9.0.

### Added

- Loop points: shift-drag on the ruler draws a loop region, double-click it to loop the whole song again. The region is saved in the project file.
- `noteCache.ts` and a `notesRevision` per track: the webview sends back the revisions it holds and the engine leaves those notes out of the snapshot.

### Changed

- Every MIDI block carries its notes in the snapshot again, so note previews show on every track.
- The render core cuts each buffer at the next note start and sample block edge, so they start on their own frame instead of up to one buffer early.
- The sample ring holds 64 messages instead of 480 016.

### Removed

- The `Begin`, `Frame` and `End` sample messages and the 15 MB pool they filled. Only tests used them; a sample now arrives whole, as an `Arc`.

## [0.10.1] - 2026-10-26

### Added

- `mise.toml`: pinned Rust and bun, plus `deps`, `install`, `dev`, `build`, `test` and `check` tasks for Linux, macOS and Windows.

### Fixed

- `tauri dev` failed on Node older than 20.19: Vite now runs on bun, so Node is not needed.
- `tauri dev` refused to start because `@tauri-apps/api` 2.11 did not match the 2.12 Rust crate; the JS packages are now 2.12.

## [0.11.0] - 2026-10-26

### Added

- Split a block at the playhead with `B` or `Ctrl/Cmd+E`; the right half of an audio block keeps playing from where the cut fell.
- Duplicate a block with `Ctrl/Cmd+D`; the copy lands right after the original.
- A left-edge handle trims a block's start while its end and its material stay in place.
- `Shift`-drag on an audio block's right edge stretches it by changing the playback rate (0.25x to 4x); a badge shows the rate.
- Right-click context menus on blocks, empty lane space and track headers, keyboard-navigable per the WAI-ARIA menu pattern; `Shift+F10` or the menu key opens the selected block's menu.
- Blocks store `offsetSeconds` and `rate`; older projects load with defaults.

### Fixed

- Starting or seeking into a sample recorded at another sample rate began at the wrong position.
- `Ctrl`/`Cmd` chords no longer play notes or fire plain-key commands.

## [0.12.0] - 2026-10-26

### Added

- Undo and redo for every edit to the song: `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` or `Ctrl+Y`, plus Undo and Redo buttons in the transport. A drag or a fader move is one step; selection, the transport and the loop region are not undone.
- A dashed hover line follows the pointer across the lanes and marks the beat a split or a click lands on, with a `bar.beat.hundredths` readout; `Alt` frees it from the grid.
- `B` splits the block under the pointer at the hover line, falling back to the selected block at the playhead.

### Fixed

- A sample kept ringing past its block after a split, duplicate or move during playback; deleting the block under a playing sample now fades it out.
- Dragging a MIDI block's left edge past a note and back deleted the note; the trim is now applied on release.
- Audio blocks drew the whole file whatever their offset, rate and length; they now draw the part they play.
- The stretch preview ran past the 0.25x to 4x rate limit and jumped on release.
- A refused split used up a block name.
- `AltGr` counted as `Ctrl`, so `AltGr+E` split a block; chords no longer type letters into the rename prompt.

## [0.13.0] - 2026-10-26

### Added

- The snap grid follows the zoom: the finest step from 1/16 beat to 2 beats that stays at least 12 px wide, drawn as faint lines on the lanes and named in the header corner.
- `Ctrl`/`Cmd` + scroll, or a trackpad pinch, zooms the timeline around the pointer.

### Changed

- The interface follows the Kono design language: the Undefined font (never bold), a black and `#111` palette with a sage green accent, square uppercase controls, dividers instead of boxes, and muted Kono track colours.
- The top bar no longer shows the "cassette" wordmark.
- The piano roll draws notes in green instead of the track colour.
