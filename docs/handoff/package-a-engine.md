# Package A: engine (effects handoff, new effects, sidechain, voices)

Rust + Tauri DAW. Read `docs/handoff/README.md` first for the shared rules.
First command: `git reset --hard <handoff commit>` and confirm with
`git log --oneline -1`.

Then read `ROADMAP.md` section "Phase 9" (Decisions: Effects, Sidechain,
Instruments), `src/core/mod.rs` (red-zone contract), `src/models/effect.rs`
(12 kinds, `Effect.id`, `Effect.sidechain`, `MAX_EFFECTS = 8`, specs for the six
new kinds), `src/models/instrument.rs` (new tone fields and `enabled`),
`src/core/effects.rs` (today: one preallocated instance per kind per track; the
six new kinds are temporary `_ =>` pass-throughs), `src/core/synth.rs`,
`src/core/render.rs`, `src/core/engine.rs` (the `AudioEngine::set_effects(&mut
self, track, &EffectChain) -> bool` seam, today a plain `RtCommand::SetEffects`
send) and `tests/test_rt_no_alloc.rs`.

## Files you may touch

`src/core/**`, `src/models/instrument.rs` (presets and their tests only; keep the
field set as the contract defines it), `tests/test_rt_no_alloc.rs`,
`tests/test_engine.rs`. Other agents edit `src/app/**`, the other
`src/models/*` files and `app/**` in parallel. The app calls only
`self.engine.set_effects(track, &chain)` for chains, plus
`RtCommand::SetEffects` on the offline export core in `src/app/export.rs`, which
builds a `RenderCore` directly. Keep `RenderCore::apply(RtCommand::SetEffects{..})`
working for that offline path, where allocation is allowed.

## Deliverable

1. **Per-effect instances with a handoff.** Replace "one instance per kind per
   track" with up to `MAX_EFFECTS` (8) instances per track; any kind may appear
   more than once. State is allocated only for effects in use, on the control
   thread, and handed to the audio thread so the callback never allocates or
   frees.
   - `AudioEngine::set_effects(track, &chain)` keeps, per track, the last chain
     it sent and diffs by `Effect.id`. For each id that is new or whose kind
     changed, it builds the instance (`Box<EffectInstance>` or similar, owning
     its buffers for the engine sample rate) and sends it. Then it sends the
     full new chain description (`EffectChain`, Copy: ids, kinds, params,
     enabled, sidechain) so the audio side can reorder its existing instances by
     id (a permutation in fixed arrays, no allocation) and drop gone ids.
   - Removed instances go back to the control thread over a return ring and the
     control side drops them (in `set_effects` and in a `collect_garbage` called
     from `send` or the app's tick path). If the return ring is full,
     `std::mem::forget` is acceptable as a last resort: a leak, never a free on
     the audio thread. Document it.
   - Use rtrb rings like the existing sample ring. Params, enabled and sidechain
     still change through the chain description. An instance whose id persists
     keeps its state, so tails survive a reorder.
   - If a ring is full, `set_effects` returns false. The app then sets
     `resync_pending`, and `AudioEngine::resync` must rebuild every track's
     chains from the song. Make sure it does.
   - The offline `RenderCore` (export): `RenderCore::new(&Song, rate)` and
     `apply(RtCommand::SetEffects{..})` may build instances directly. Keep one
     code path for the DSP itself.
   - Remove the old per-kind arrays and the temporary `_ =>` arms.
2. **Six new effects**, per the spec table in `effect.rs`:
   - Chorus: Rate Hz, Depth, Delay ms, Mix. Two modulated delay lines, LFOs in
     quadrature for stereo.
   - Phaser: Rate, Depth, Feedback, Mix. 4 to 6 first-order allpass stages with
     an LFO-swept coefficient; offset the stereo LFO phase.
   - Auto Pan: Mode Pan/Tremolo, Rate in beats (tempo-synced through
     `set_tempo`), Depth.
   - Redux (`BitCrusher`): Bits 2..16, Downsample 1..32 (sample and hold), Mix.
   - Utility: Gain dB, Width (0 mono, 1 normal, 2 wide, mid/side), Pan.
   - Limiter: Gain dB in, Ceiling dB, Release ms. Peak limiter, instant attack,
     optional short lookahead in a fixed preallocated buffer. Never exceed the
     ceiling by more than a tiny epsilon.
   - Chorus and Phaser count as tails in `is_ringing` (`EffectKind::has_tail`).
3. **Sidechain**, Ableton-style compressor sidechain. For an effect with
   `kind.takes_sidechain()` (the Compressor) and `sidechain: Some(src)` where
   `src != own track` and `src < track_count`, the detector reads the SOURCE
   track's key signal instead of its own input. The key is the source track's
   post-chain, pre-fader stereo from the PREVIOUS frame, so processing order does
   not matter: keep a fixed `[(f32, f32); MAX_TRACKS]` of last-frame outputs in
   `SynthEngine`. It is taken regardless of the source's mute and solo state, so
   a muted ghost kick still drives the ducking. The audio being compressed is
   still the track's own. An out-of-range or own-track source falls back to
   self-detection. Test: a loud pulse on track 0 with track 1's compressor
   sidechained to 0 reduces track 1's sustained tone during the pulse and not
   after release.
4. **Instrument voice features** in `synth.rs`, every new field read from
   `Instrument`:
   - `unison` 1..=7 detuned oscillators spread over `detune` cents, level
     normalized.
   - `sub`: a sine one octave down.
   - `cutoff` / `resonance`: a per-voice resonant low-pass (TPT SVF). 20000 Hz
     means bypass, bit-exact to today's output.
   - `filter_env`: octaves above cutoff, following the amp envelope.
   - `pitch_env` / `pitch_decay`: a semitone sweep falling exponentially to the
     note, for 808s and toms.
   - `fm_ratio` / `fm_index`: a sine modulator into the oscillator phase. Index
     0 means off.
   - `enabled: false` means silent voices, and a silent sample voice for audio
     tracks, with a short fade so there is no click.
   - Precompute coefficients where you can. Clamp every field defensively.
   - **The default Lead and every existing preset must sound bit-exact as
     before.** Snapshot the old output before you change the code and compare.
5. **New presets** in `src/models/instrument.rs`. Extend `Instrument::PRESETS`
   and `preset()` with: Supersaw, Sub Bass, Acid, Keys (FM electric piano), Bell
   (FM), Organ, Strings, Brass, 808, Tom, Clap, Rim. Keep the existing 7
   unchanged and first. Design each so it sounds like its name. Keep
   `preset_index` working. Add a `PRESET_GROUPS` table or
   `pub const fn group(name) -> &'static str` mapping each preset to "Synth",
   "Keys", "Bass", "Pad" or "Drums". Do not edit `src/ui/view.rs`; expose the
   table in `instrument.rs` and say so in your report.
6. **Tests.** Every new effect does something measurable and is bit-exact when
   disabled. Two Delays on one track both run. A reorder keeps a reverb tail.
   Removal returns the box to the control side. `tests/test_rt_no_alloc.rs`
   covers rendering with 8 effects including duplicates and the sidechain, plus
   applying chain and handoff messages on the audio thread, under
   `assert_no_alloc`. Unison, sub, filter, pitch env, FM and bypass each have a
   test. Old presets are bit-exact.

## Gates and commits

`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Do not
weaken existing tests; if an app-side test breaks because of your change, report
it instead of editing `src/app`. Commit with Conventional Commits, for example
`feat(core): hand effect state to the audio thread and allow duplicate kinds`,
`feat(core): add chorus, phaser, auto pan, redux, utility and limiter`,
`feat(core): sidechain the compressor to another track`,
`feat(core): widen the voice and add twelve presets`.

Final report, under 300 words: the handoff design (message types, ring sizes,
the garbage path), public API changes with exact signatures, memory per effect
kind, what needs wiring at merge, the test count, the commit hashes and branch.
