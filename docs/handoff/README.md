# Handoff: Phase 9 (editing, arrangement, sound library)

Written 2026-10-27 so any agent can finish the release if the original session
runs out of budget. Read this file first, then `ROADMAP.md` section "Phase 9".

## What the user asked for (verbatim intent)

1. Piano roll: resize notes from the start (left edge), not only the end.
2. Piano roll: select a region with the mouse (marquee) to move several notes
   at once or delete them.
3. Track view: draw selection zones (time selection across tracks) and operate
   on them: cut, copy, paste, duplicate, delete, and loop over the selection.
4. Review the app UX against Ableton and apply the required changes (done: the
   findings are in ROADMAP.md Phase 9 "UX review").
5. More default effects and instruments.
6. Save on a never-saved project must let the user pick the path. Project
   files get their own extension: `.cassette` (JSON inside; `.json` still loads).
7. A sidechain effect like Ableton's compressor sidechain, so a bass can duck
   to a kick.
8. Then `/ship` the new version (v0.15.0).

## State of the repo

- Branch `feat/arrangement`, based on `main` at v0.14.0.
- Commit `4538e21` is the type contract: Rust models, events, snapshot fields
  and their TypeScript twins, all compiling with stub handlers, all gates green.
- The four work packages below are independent and touch disjoint files.
  Each has its own spec file. Run them in parallel worktrees, merge in the
  order A, B, C, D, then run the integration steps at the end of this file.

| Package | Spec | Owns |
|---------|------|------|
| A engine | `package-a-engine.md` | `src/core/**`, `src/models/instrument.rs`, `tests/test_rt_no_alloc.rs`, `tests/test_engine.rs` |
| B app logic | `package-b-app.md` | `src/app/**`, `src/models/{effect,track,song,note}.rs`, parts of `src/ui/view.rs` |
| C piano roll | `package-c-piano-roll.md` | `app/src/lib/components/PianoRoll.svelte`, `app/src/lib/pianoRoll.ts` (+ test), `app/src/lib/noteSelection.ts` (+ test) |
| D arrangement UI | `package-d-arrangement-ui.md` | every other file under `app/src/**` and `app/src-tauri/**` |

## Rules that apply to every package

- No inline comments and no comments inside function bodies. `///` or JSDoc
  above public items only. No dead code. Match the surrounding style (Rust:
  rustfmt; frontend: tabs, single quotes, width 100, no trailing commas, no
  prettier config exists so never run `prettier --write`).
- Never use the em dash. Never use emoji in code or docs.
- No co-author trailer and no "Generated with" line in commits. Conventional
  Commits: `type(scope): summary`, imperative, lowercase, under 72 characters.
- Gates (repo root): `mise run check` and `mise run test`. These run
  `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, the Tauri
  shell's fmt and clippy, `cd app && bun run check` (0 errors, 0 warnings) and
  `cargo test` plus `cd app && bun test`. No `filet.yml` exists, so filet does
  not apply.
- Red zone: nothing on the audio thread allocates, locks, does I/O or panics.
- Agent worktrees start on `main`, not on the feature branch. The first command
  of every agent must be `git reset --hard <contract-or-handoff commit>`.

## Integration steps after all four packages merge

1. `git merge` A, B, C, D into `feat/arrangement` in that order. Expect small
   conflicts only in `ROADMAP.md` or lockfiles.
2. Wire what the packages could not: the preset groups table from
   `src/models/instrument.rs` into the snapshot (`src/ui/view.rs`) and
   `app/src/lib/wire.ts`, if package D asks for it.
3. Run both gates. Fix whatever fails.
4. Add an end-to-end test: an effect with a sidechain added through the app
   changes the exported WAV (see `an_added_delay_echoes_in_the_export` in
   `src/app/export.rs` for the pattern, including the mutation check: remove
   the thing under test and confirm the test fails).
5. Headless screenshot to eyeball the UI: see the wiki page
   `~/.mycelium/memory/projects/cassette.md` ("stub Tauri for screenshots").
6. Docs: `CHANGELOG.md` (new `[0.15.0]` section: Added / Changed / Fixed),
   `README.md` controls table, `ROADMAP.md` Phase 9 status and "departures".
7. Ship v0.15.0, manually (no CI): bump `Cargo.toml`, `app/package.json`,
   `app/src-tauri/Cargo.toml`, `app/src-tauri/tauri.conf.json`, and the two
   lockfile entries (`Cargo.lock`, `app/src-tauri/Cargo.lock`), commit
   `chore(release): v0.15.0` on `main` after a fast-forward merge, tag `v0.15.0`,
   push `main` and the tag, then `gh release create v0.15.0 --notes-file` with the
   changelog section. The v0.14.0 release followed exactly this recipe.
8. Remove the agent worktrees and merged branches, and add a wiki note with
   `mycelium memory add projects/cassette.md ...`.

## Known gaps to mention in the changelog if unfinished

- Nobody has heard the new effects, instruments, metronome or sidechain in a
  running window. Say so in the release notes.
