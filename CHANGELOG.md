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

## [0.13.1] - 2026-10-26

### Fixed

- Sliders drew the webview's rounded native thumb and track; they are now square, with a 2 px track that fills green up to the value and a square green thumb.

## [0.14.0] - 2026-10-27

### Added

- MIDI recording: `F9` or the Rec button arms it and starts the transport. Played notes land in the MIDI block under the playhead, or in a new block that starts on the note's bar and grows bar by bar as the take continues. A take is one undo step, and a hardware MIDI keyboard records the same way.
- A marker, like Ableton's insert marker: double-clicking an empty spot on a lane selects that track and puts the marker on the snapped beat. Play starts from the marker, `Stop` returns to it, and a second `Stop` goes back to the top.
- A detail pane under the timeline, modelled on Ableton's Detail View. It has a Clip view (the piano roll) and a Devices view (the selected track's device chain). `Shift+Tab` switches between them. Selecting a track shows its devices, and opening a MIDI block shows its clip. Drag the top edge to resize the pane; the size and the open state are remembered.
- Six built-in insert effects: EQ Three, Filter, Saturator, Compressor, Delay (tempo-synced) and Reverb (Freeverb). Each device card has an on/off activator, fold, delete, and dials. Drag a dial or scroll over it, `Shift` gives fine steps, and a double-click resets it. Drag a device's title bar to reorder the chain. Each kind can appear once per track. The chain is saved with the project, rendered in export, and every edit is undoable (one step per dial drag).
- The sidebar's Effects tab is now the effect browser. Double-click an effect to add it to the selected track, or drag it into the chain.
- A metronome (`Metro`) clicks on every beat, accented on the downbeat. It is never part of an export. Its `▾` menu sets the count-in (none, 1, 2 or 4 bars; 1 by default) and record quantize (off, 1/4, 1/8, 1/16).
- Recording from stop counts in first. The position readout shows the beats left, and notes played before the downbeat are not recorded.
- Quantize: `Ctrl+U` (`Cmd+U` on macOS) or the piano roll's Quantize button snaps note starts to the grid and keeps their lengths.

### Changed

- Double-clicking an empty lane places the marker instead of creating a block; the lane's right-click menu still has "New block here".
- The Rec button records notes. It used to bounce the project to a WAV, which `Export` already does.
- The voice editor moved from the sidebar into the Instrument device, the first card of a MIDI track's chain.

### Fixed

- Keyboard notes went silent after half a second while the key was still held: a timeout left over from the terminal interface released every note after 500 ms.

## [0.15.0] - 2026-10-27

### Added

- Arrangement time selection: drag across empty lane space to select a time range across tracks, with edge handles to adjust the span and a length readout in the header corner.
- Arrangement selection commands: `Ctrl/Cmd+C` copy, `Ctrl/Cmd+X` cut, `Ctrl/Cmd+V` paste at marker, `Ctrl/Cmd+D` duplicate, `Delete`/`Backspace` delete range without shifting time, and `Ctrl/Cmd+L` loop selection. A context menu on the selection band mirrors the commands.
- Track management: add audio track (`Ctrl/Cmd+T`), add MIDI track (`Ctrl/Cmd+Shift+T`), plus a `+` button in the track header column, right-click on empty timeline space to add tracks, and track context menu to rename, duplicate, or delete up to 16 tracks.
- Piano roll marquee selection: drag empty grid to marquee-select notes, Shift+marquee to add to the selection, Shift+click to toggle individual notes, `Ctrl/Cmd+A` select all, `Esc` clear.
- Piano roll multi-note editing: moving or resizing any selected note shifts the entire selection while preserving relative pitch, timing, and chord shape.
- Piano roll start-edge resizing: dragging the left edge of a note changes its start and length while pinning the end in place.
- Piano roll draw mode: toggle with the pencil button or `B` key on the grid to click-drag new notes and click to delete existing notes.
- Piano roll deep zoom: zoom in up to 2048 px/beat with the zoom buttons, `+`/`-` keys, or wheel while holding `Ctrl`, `Meta`, `Alt`, or hovering over empty grid.
- Piano roll note clipboard: `Ctrl/Cmd+C`, `Ctrl/Cmd+X`, `Ctrl/Cmd+V`, and `Ctrl/Cmd+D` for selected notes.
- Six new insert effects: Chorus, Phaser, Auto Pan, Redux (BitCrusher), Utility (Gain, Width, Pan), and Limiter.
- Multi-instance insert effects: up to 8 effects per track, allowing multiple instances of the same effect kind. State is allocated on the control thread and handed to the audio thread over lock-free rings.
- Compressor sidechain: the compressor can listen to another track's pre-fader post-chain signal from the previous frame to duck the audio. A muted ghost track still drives the ducking.
- Expanded instrument voice features: unison detune (1 to 7 voices), sub oscillator, resonant low-pass filter (TPT SVF) with envelope sweep, pitch sweep envelope, two-operator FM synthesis, and instrument bypass with four Tone dials (Cutoff, Resonance, Detune, Sub).
- Twelve new built-in presets: Supersaw, Sub Bass, Acid, Keys, Bell, Organ, Strings, Brass, 808, Tom, Clap, Rim.
- Instruments browser: sidebar tab renamed to Instruments, grouping the 19 presets by role (Synth, Keys, Bass, Pad, Drums).
- Project files: projects use the `.cassette` extension (JSON inside; `.json` files still load). Save prompts for a path on untitled projects, and `Ctrl/Cmd+Shift+S` opens Save As. Unsaved changes are marked with a dot next to the project name in the transport.

### Changed

- Saving an untitled project prompts with a native save file dialog to pick destination before saving.

### Fixed

- Live keyboard notes were cut off when the playback loop wrapped around: loop wrap now uses `release_timeline_notes()` to cut only sequenced timeline notes while held keyboard notes continue to sound.
- Piano roll zoom was previously clamped to timeline scale (256 px/beat), preventing deep zoom on short notes.
- Nobody has heard the new effects, instruments, metronome or sidechain in a running window.

## [0.15.1] - 2026-10-27

### Added

- Arrow key navigation in arrangement: ArrowUp and ArrowDown navigate tracks or nudge the selected block; ArrowLeft and ArrowRight seek the playhead or nudge the selected block by grid steps.
- Export progress indicator: progress percentage and progress bar during audio export, with disabled loading state on the Export button.
- Save loading status: disabled loading indicator on the Save button while saving project files.

### Changed

- Widened the left sidebar tab bar from 248 px to 280 px with nowrap labels so the Effects tab text is never clipped.
- Piano roll scrolling: scrolling over the note grid scrolls the view normally; Ctrl/Cmd+scroll zooms; Alt+scroll over a note adjusts its velocity.

### Fixed

- Voice release on note cut: synth voices now track active note updates and release immediately when a MIDI block or note is cut, split, or shortened during playback.
- Block drag and ghost selection overlay: block dragging no longer spawns a stationary selection rectangle over the lane that blocked clicks or lost drag focus.
- Selection band z-index: arrangement selection band and resize handles now render below sticky track headers and ruler instead of painting over the track UI at the right.
- Double-click to open MIDI blocks: double-clicking a block reliably opens the piano roll.
- Double-click reset on controls: double-clicking track volume/pan faders, voice envelope/gain/octave sliders, or tone and effect knobs resets them to their default values.

## [0.15.2] - 2026-10-27

### Fixed

- Audio export progress reset: export button and progress indicator now properly reset to idle once rendering completes instead of staying stuck at 100%.
- Track level metering post-fader: channel volume meters now scale with the track volume fader instead of ignoring it and displaying pre-fader insert chain peaks.

## [0.15.3] - 2026-10-27

### Fixed

- Track deletion playback continuity: deleting a track while playback is active preserves sounding voices, sample playback, and effect chains on surviving tracks instead of cutting audio off.
- Transport stop and lock during export: audio export stops active playback and disables transport controls during rendering to avoid concurrent playback collisions.

## [0.16.0] - 2026-10-27

### Added

- Track drag-and-drop reordering: drag tracks vertically by their grip handle in the track header list to reorder, with a visual drop seam indicator and zero-allocation in-place audio mirror rotation that preserves active audio playback without stops or glitches.
- Persistent track color customization: tracks initialize with order-based colors from the 8-color Kono palette and store their color across moves and session saves.
- Track recoloring context menu and keybind: right-click any track header to select from the 8-color palette swatches, or press `Alt+C` to quickly open the recolor options for the selected track.
- Effect card selection highlighting: clicking or focusing an effect device highlights its outline in Kono green.
- Slider and dial arrow key navigation: `ArrowUp`, `ArrowRight`, `ArrowDown`, and `ArrowLeft` adjust focused volume/pan faders, voice envelope sliders, and device dials without triggering DAW global transport or block navigation.
- Effect clipboard and duplication: `Ctrl/Cmd+C` copies the selected effect including its parameter values and state, `Ctrl/Cmd+V` pastes after the selection or at the end of the chain, and `Ctrl/Cmd+D` duplicates the effect, with matching options in the device context menu.
- Multi-platform desktop release workflow: GitHub Actions workflow building and packaging Windows x64 setup installers, macOS Universal DMGs, and Linux x64 AppImage/deb packages with SHA-256 checksums.
- Cassette official website: SvelteKit landing page and download portal with live GitHub release detection, styled with the Kono design system and deployed to `cassette.facile.studio`.

## [0.16.1] - 2026-10-27

### Changed

- Landing page design aligned with Vitrine studio style: solid green callout section with borderless layout, dark action button, and streamlined typography.
- Pixelarticons integration: replaced plain text symbols with pixelarticons across navigation, feature cards, downloads, and technical specifications.

### Removed

- Live session view simulation block from the landing page.

## [0.16.2] - 2026-10-27

### Changed

- Typography consistency: enforced the Undefined font universally across all elements and code blocks on the website and desktop application.
- Desktop application identity: updated app icon across macOS, Windows, Linux, iOS, and Android to `pixelarticons:keyboard-music` in `#88A888` on `#000000`.
- Website branding: updated website logo and primary green theme color to `#88A888` on `#000000` background.

## [0.16.3] - 2026-10-27

### Added

- Interactive application preview on the website showcasing the Devices & Instruments rack and Piano Roll & Effects editor.
- High-resolution session screenshots in the repository README and website static assets.
- Morphing responsive top navigation matching Kono Vitrine with scroll transitions and slide-out mobile drawer.

### Changed

- Desktop application icon: generated assets adhering to Apple HIG safe-area squircle specifications with centered 24x pixel-aligned `pixelarticons:keyboard-music` in `#88A888` on `#000000`.
- Website logo: updated `logo.svg` to pure `pixelarticons:keyboard-music` vector in `#88A888` without clipping boxes.
- Borderless navigation styling: removed borders around the top navigation and mobile sidebar drawer.

### Fixed

- Undefined font consistency: preloaded font in `app.html`, mapped all weights (`100 900`, `normal`, `bold`) in `@font-face`, and removed font-weight overrides.
- Top right action button styling: removed duplicate icon artifact and streamlined layout.
- Favicon pixel-art fidelity: updated vector SVG favicon (`favicon.svg`) and raster PNG assets (`favicon.png`, `apple-touch-icon.png`, `icon.png`) to render `#88A888` keyboard-music with transparent background.

## [0.17.0] - 2026-10-27

### Added

- EQ Eight visual parametric equalizer: 8-band parametric EQ with interactive biquad frequency response curve, live animated audio wave spectrum, and draggable nodes (frequency, gain, and Q bandwidth adjustment). Filter types include Low Cut, Low Shelf, Bell, High Shelf, High Cut, and Notch.
- Enhanced Stereo Utility effect: 400% stereo width scaling, dedicated routing modes (Stereo, Mono, Left, Right), and a sub-bass mono crossover filter to center low-end frequencies while preserving high stereo width.
- Punchy kick instrument preset: redesigned kick with 36-semitone pitch sweep, rapid 26ms envelope drop, transient FM beater click, and deep sub-bass foundation.
- Master bus analog saturation: 4 non-linear waveshaping models (Tape, Tube, Console, Diode) with continuous tone tilt filtering and low-end warmth enhancement.
- Ableton-style vertical Mixer console: Tab key toggles between horizontal arrangement timeline and vertical mixer view with dB faders, LED peak meters, pan sliders, insert effect chips, and pinned master strip.
- Multi-track selection: Shift-click for range selection, Cmd/Ctrl-click for multi-track toggle, with bulk mute, solo, delete, duplicate, and recolor actions.
- Audio track microphone recording: record live mic input directly into armed audio tracks as 16-bit PCM WAV blocks.
- Cross-track drag guard: prevents dragging MIDI blocks to audio tracks and vice versa with visual not-allowed feedback and red outline.
- Desktop settings dialog and self-updater: audio device status, keyboard shortcut reference, and in-app self-updater powered by Tauri updater plugin with automatic release checks.
- Stem export: renders each populated track soloed to an individual WAV file preserving sidechain signals.

### Fixed

- Piano roll Space key: pressing Spacebar in the MIDI editor now toggles playback instead of scrolling the view.

## [0.17.1] - 2026-10-27

### Fixed

- Tauri updater ACL permissions: grant updater and process permissions in desktop capability configuration to allow update checks, downloads, and app restarts.
- Mixer view retro geometry: remove border rounding from pan slider knobs, vertical level meters, and channel strip chips to match the square Kono theme.
