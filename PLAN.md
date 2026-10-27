# Plan: Cassette Track Reordering, Track Colors, Effect Copy/Paste & Vitrine Deployment

## 1. Type Contract & Core Engine Foundations
- `src/models/track.rs`:
  - Add `color_index: Option<u8>` to `Track` (serde default).
  - Initialize new tracks with `(index % 8) as u8`.
  - Preserve `color_index` during reordering and project serialization.
- `src/events/types.rs`:
  - Add `Event::ReorderTrack { from: usize, to: usize }`
  - Add `Event::SetTrackColor { track: usize, color_index: u8 }`
  - Add `Event::PasteEffect { track: usize, index: Option<usize>, kind: EffectKind, params: Vec<f32>, enabled: bool, sidechain: Option<usize> }`
  - Add `Event::DuplicateEffect { track: usize, index: usize }`
- `src/core/render.rs` & `src/core/synth.rs`:
  - Add `RtCommand::MoveTrack { from: u8, to: u8 }`
  - Implement zero-alloc in-place rotation for block mirrors, note mirrors, active synth tracks, last frame outputs, and current frame outputs.
  - Zero audio glitch or playback interruption during track reordering.
- `src/app/tracks.rs` & `src/app/effects.rs` & `src/app/history.rs`:
  - Implement `reorder_track`, `set_track_color`, `paste_effect`, `duplicate_effect`.
  - Sidechain remapping for `TrackShift::Moved { from, to }`.
  - Record undo checkpoints for all operations.
- `src/ui/view.rs` & `app/src/lib/wire.ts`:
  - Expose `color_index` on `TrackView`.
  - Expose new event variants in `EngineEvent`.

## 2. Desktop UI & Interaction
- Track Drag Reordering (`TrackHeader.svelte`, `Timeline.svelte`):
  - Vertical drag handle and pointer drag on track header.
  - Visual insertion seam between track headers during drag.
  - Dispatches `reorderTrack` on drop without audio glitch.
- Track Color Customization (`ContextMenu.svelte`, `menu.ts`, `TrackHeader.svelte`, `keys.ts`):
  - Minimal Kono theme color swatches row in track context menu.
  - Quick keybind `Alt+C` opens track recolor options for selected track.
- Effect Selection & Highlighting (`DeviceChain.svelte`, `Device.svelte`):
  - Selecting an effect highlights its border (`var(--green)`).
  - Deselect on click outside.
- Slider & Knob Arrow Navigation (`Knob.svelte`, `TrackHeader.svelte`, `VoiceEditor.svelte`, `keyboard.ts`):
  - Prevent DAW global shortcuts from hijacking arrow keys when sliders or dials are focused.
  - Allow precise arrow key increment/decrement with shift support.
- Effect Copy & Paste (`DeviceChain.svelte`, `Device.svelte`, `session.svelte.ts`, `menu.ts`):
  - `Ctrl+C` copies selected effect state (kind, params, enabled, sidechain).
  - `Ctrl+V` pastes after selected effect or at end of chain.
  - `Ctrl+D` duplicates effect.
  - Right-click device context menu options for Copy, Paste, Duplicate.

## 3. GitHub Actions Release Workflow
- `.github/workflows/desktop.yml`:
  - Matrix builds: windows-x64, macos-universal, linux-x64 (AppImage + deb).
  - Packaged and published on tag `v*` with checksums.

## 4. Cassette SvelteKit Website & Dokploy Deployment
- `website/`:
  - Clean SvelteKit landing page and `/download` page matching Fusible Vitrine / Kono style.
  - OS detection and direct release asset download links.
  - Dockerfile for production build with `@sveltejs/adapter-node`.
- Dokploy Deployment:
  - Project `Cassette` and Application `Cassette Web`.
  - Domain `cassette.facile.studio` with Let's Encrypt HTTPS.

## 5. Verification & Ship
- Run unit and integration tests (`cargo test`, `test_rt_no_alloc`).
- Run TypeScript / Svelte checks (`mise run check`).
- Bump version to `0.16.0`.
- Release with `/ship`.
