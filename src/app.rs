//! The application: terminal lifecycle, the event loop and command handling.
//!
//! The loop is deliberately small — poll input, drain the event bus, apply
//! intent to the song and the engine, publish a [`UiState`] snapshot, draw.
//! All timing and synthesis live in [`crate::core`].

use std::io;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam::channel::Receiver;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::core::MAX_BLOCKS;
use crate::core::engine::AudioEngine;
use crate::core::meter::MeterBank;
use crate::core::render::{BlockName, RtCommand};
use crate::events::Event;
use crate::events::bus::EventBus;
use crate::input::InputHandler;
use crate::models::{Instrument, Song, TrackKind};
use crate::ui::layout::Pane;
use crate::ui::renderer;
use crate::ui::view::{Cursor, TrackView, UiState, View};
use crate::utils::time::Tempo;

/// How long a played note rings when the terminal cannot report releases.
const HELD_NOTE_MS: u64 = 500;
/// UI frame budget.
const FRAME_MS: u64 = 16;

/// A note waiting to be released.
struct HeldNote {
    track: u8,
    pitch: u8,
    deadline: Instant,
}

/// The running application.
pub struct App {
    song: Song,
    engine: AudioEngine,
    bus: EventBus,
    events: Receiver<Event>,
    input: InputHandler,
    cursor: Cursor,
    focus: Pane,
    meters: MeterBank,
    held: Vec<HeldNote>,
    octave: i32,
    /// Buffer of the block-name prompt, when one is open.
    rename: Option<String>,
    status: String,
    show_help: bool,
    running: bool,
    resync_pending: bool,
}

impl App {
    /// Build an app with the live audio backend.
    pub fn new() -> Result<Self> {
        let song = Song::new();
        let engine = AudioEngine::start(&song).context("starting the audio engine")?;
        Ok(Self::with(song, engine))
    }

    /// Build an app that streams audio over loopback TCP (audio over SSH).
    pub fn new_streaming(port: u16) -> Result<Self> {
        let song = Song::new();
        let engine =
            AudioEngine::start_streaming(&song, port).context("starting the streaming engine")?;
        Ok(Self::with(song, engine))
    }

    /// Build an app whose engine makes no sound (headless / CI).
    pub fn new_headless() -> Result<Self> {
        let song = Song::new();
        let engine = AudioEngine::start_silent(&song).context("starting the silent engine")?;
        Ok(Self::with(song, engine))
    }

    fn with(song: Song, engine: AudioEngine) -> Self {
        let mut bus = EventBus::new();
        let events = bus.subscribe();
        let meters = MeterBank::new(song.tracks.len());
        let status = format!(
            "audio '{}' at {} Hz — {} · press ? for help",
            engine.backend_name(),
            engine.sample_rate(),
            engine.note()
        );
        Self {
            song,
            engine,
            bus,
            events,
            input: InputHandler::new(),
            cursor: Cursor::default(),
            focus: Pane::Tracks,
            meters,
            held: Vec::new(),
            octave: 0,
            rename: None,
            status,
            show_help: false,
            running: true,
            resync_pending: false,
        }
    }

    /// Run the full-screen terminal UI until the user quits.
    pub fn run() -> Result<()> {
        let mut terminal = setup_terminal()?;
        let _guard = TerminalGuard;
        let mut app = App::new()?;
        app.event_loop(&mut terminal)
    }

    /// Run the terminal UI while streaming audio on `port` for a local player.
    ///
    /// Forward the port over SSH and play the stream on your own machine; see
    /// the README section *Audio over SSH*.
    pub fn run_streaming(port: u16) -> Result<()> {
        let mut terminal = setup_terminal()?;
        let _guard = TerminalGuard;
        let mut app = App::new_streaming(port)?;
        app.event_loop(&mut terminal)
    }

    /// Print a single plain-text frame and exit. Exercises the alternate
    /// renderer without needing a terminal or an audio device.
    pub fn run_headless() -> Result<()> {
        let app = App::new_headless()?;
        println!("{}", app.ui_state().render());
        Ok(())
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        while self.running {
            for event in self.input.poll(Duration::from_millis(FRAME_MS)) {
                self.bus.send(event);
            }
            while let Ok(event) = self.events.try_recv() {
                self.handle(event);
            }
            if self.resync_pending {
                self.engine.resync(&self.song);
                self.resync_pending = false;
            }
            self.release_expired_notes();

            let peaks = self.engine.shared().peaks();
            self.meters.update(&peaks);

            let state = self.ui_state();
            terminal.draw(|frame| renderer::draw(frame, &state))?;
        }
        Ok(())
    }

    // -- event handling ----------------------------------------------------

    fn handle(&mut self, event: Event) {
        match event {
            Event::Quit => self.running = false,
            Event::TogglePlay => self.toggle_play(),
            Event::Stop => {
                self.send(RtCommand::Stop);
                self.status = "stopped".into();
            }
            Event::ToggleLoop => {
                let looping = !self.engine.shared().looping.load(Ordering::Relaxed);
                self.send(RtCommand::SetLooping { looping });
                self.status = format!("looping {}", if looping { "on" } else { "off" });
            }
            Event::ToggleHelp => self.show_help = !self.show_help,
            Event::MoveTrack(delta) => self.move_track(delta),
            Event::MoveBlock(delta) => self.move_block(delta),
            Event::AddBlock => self.add_block(),
            Event::DeleteBlock => self.delete_block(),
            Event::CycleTrackKind => self.cycle_track_kind(),
            Event::RenameBlock => self.rename_block(),
            Event::TextInput(character) => self.type_character(character),
            Event::TextBackspace => {
                if let Some(buffer) = self.rename.as_mut() {
                    buffer.pop();
                    self.status = format!("rename: {buffer}");
                }
            }
            Event::CancelRename => self.cancel_rename(),
            Event::VolumeDelta(delta) => self.nudge_volume(delta),
            Event::PanDelta(delta) => self.nudge_pan(delta),
            Event::BpmDelta(delta) => self.nudge_bpm(delta),
            Event::ToggleMute => self.toggle_mute(),
            Event::ToggleSolo => self.toggle_solo(),
            Event::OctaveDelta(delta) => self.nudge_octave(delta),
            Event::NoteOn { pitch, velocity } => self.play_note(pitch, velocity),
            Event::NoteOff { pitch } => self.release_note(pitch),
            Event::Resize(_, _) => {}
            // OSC control is routed by adapters; no binding yet.
            Event::OscControl { address, value } => {
                self.status = format!("osc {address} = {value:.2}");
            }
        }
    }

    fn toggle_play(&mut self) {
        if self.engine.is_playing() {
            self.send(RtCommand::Stop);
            self.status = "stopped".into();
        } else {
            self.send(RtCommand::Play);
            self.status = "playing".into();
        }
    }

    // -- session editing ---------------------------------------------------

    fn move_track(&mut self, delta: i32) {
        let tracks = self.song.tracks.len().max(1) as i32;
        self.cursor.track = (self.cursor.track as i32 + delta).clamp(0, tracks - 1) as usize;
        self.clamp_block();
    }

    fn move_block(&mut self, delta: i32) {
        let blocks = self.block_slots().max(1) as i32;
        self.cursor.block = (self.cursor.block as i32 + delta).clamp(0, blocks - 1) as usize;
    }

    fn add_block(&mut self) {
        let track_index = self.cursor.track;
        let Some(track) = self.song.tracks.get_mut(track_index) else {
            return;
        };
        if track.blocks.len() >= MAX_BLOCKS {
            self.status = format!(
                "track {} is already at {MAX_BLOCKS} blocks",
                track_index + 1
            );
            return;
        }
        let index = track.add_block();
        let name = track.blocks[index].name.clone();
        self.send(RtCommand::AddBlock {
            track: track_index as u8,
            name: BlockName::new(&name),
        });
        self.cursor.block = index;
        self.status = format!("block {name} added to track {}", track_index + 1);
    }

    fn delete_block(&mut self) {
        let track_index = self.cursor.track;
        let block = self.cursor.block;
        let Some(track) = self.song.tracks.get_mut(track_index) else {
            return;
        };
        if block >= track.blocks.len() {
            self.status = "move onto a block to delete it".into();
            return;
        }
        let removed = track.blocks.remove(block);
        self.clamp_block();
        // The audio side only mirrors appends and pops, so a removal from the
        // middle desyncs it; rebuild the whole mirror instead of guessing.
        self.resync_pending = true;
        self.status = format!("block {} deleted", removed.name);
    }

    fn cycle_track_kind(&mut self) {
        let track_index = self.cursor.track;
        let Some(track) = self.song.tracks.get_mut(track_index) else {
            return;
        };
        track.kind = match track.kind {
            TrackKind::Midi => TrackKind::Audio,
            TrackKind::Audio => TrackKind::Midi,
        };
        track.instrument = Instrument::preset(match track.kind {
            TrackKind::Midi => "Lead",
            TrackKind::Audio => "Pluck",
        });
        let (kind, instrument, volume, pan, muted, solo) = (
            track.kind,
            track.instrument,
            track.volume,
            track.pan,
            track.muted,
            track.solo,
        );
        self.send(RtCommand::SetTrackInstrument {
            track: track_index as u8,
            instrument,
        });
        self.send_mix(track_index, volume, pan, muted, solo);
        self.status = format!("track {} is now {}", track_index + 1, kind.name());
    }

    fn rename_block(&mut self) {
        if self.rename.is_some() {
            self.rename_commit();
            return;
        }
        let Some(name) = self
            .song
            .tracks
            .get(self.cursor.track)
            .and_then(|track| track.blocks.get(self.cursor.block))
            .map(|block| block.name.clone())
        else {
            self.status = "move to a block to rename it".into();
            return;
        };
        self.rename = Some(String::new());
        self.set_text_mode(true);
        self.status = format!("renaming {name}: type a new name, enter to confirm, esc to cancel");
    }

    fn rename_commit(&mut self) {
        let Some(buffer) = self.rename.take() else {
            return;
        };
        self.set_text_mode(false);
        let name = buffer.trim().to_string();
        if name.is_empty() {
            self.status = "rename left unchanged".into();
            return;
        }
        let track_index = self.cursor.track;
        let block_index = self.cursor.block;
        if let Some(slot) = self
            .song
            .tracks
            .get_mut(track_index)
            .and_then(|track| track.blocks.get_mut(block_index))
        {
            slot.name = name.clone();
        }
        // The audio side only mirrors appends, so a rename resyncs the names.
        self.resync_pending = true;
        self.status = format!("block renamed to {name}");
    }

    fn cancel_rename(&mut self) {
        if self.rename.take().is_some() {
            self.set_text_mode(false);
            self.status = "rename cancelled".into();
        }
    }

    fn type_character(&mut self, character: char) {
        let Some(buffer) = self.rename.as_mut() else {
            return;
        };
        if buffer.len() + character.len_utf8() <= BlockName::CAPACITY {
            buffer.push(character);
            self.status = format!("rename: {buffer}");
        }
    }

    // -- mixer -------------------------------------------------------------

    fn nudge_volume(&mut self, delta: f32) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get_mut(track) else {
            return;
        };
        target.volume = (target.volume + delta).clamp(0.0, 1.0);
        let (volume, pan, muted, solo) = (target.volume, target.pan, target.muted, target.solo);
        self.send_mix(track, volume, pan, muted, solo);
        self.status = format!("volume {volume:.2}");
    }

    fn nudge_pan(&mut self, delta: f32) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get_mut(track) else {
            return;
        };
        target.pan = (target.pan + delta).clamp(-1.0, 1.0);
        let (volume, pan, muted, solo) = (target.volume, target.pan, target.muted, target.solo);
        self.send_mix(track, volume, pan, muted, solo);
        self.status = format!("pan {:+.2}", pan);
    }

    fn nudge_bpm(&mut self, delta: f32) {
        self.song.set_bpm(self.song.bpm() + delta);
        self.send(RtCommand::SetBpm {
            bpm: self.song.bpm(),
        });
        self.status = format!("bpm {:.0}", self.song.bpm());
    }

    fn toggle_mute(&mut self) {
        let track = self.cursor.track;
        let (name, volume, pan, muted, solo) = {
            let Some(target) = self.song.tracks.get_mut(track) else {
                return;
            };
            target.muted = !target.muted;
            (
                target.name.clone(),
                target.volume,
                target.pan,
                target.muted,
                target.solo,
            )
        };
        self.send_mix(track, volume, pan, muted, solo);
        self.status = format!("{name} {}", if muted { "muted" } else { "unmuted" });
    }

    fn toggle_solo(&mut self) {
        let track = self.cursor.track;
        let (name, volume, pan, muted, solo) = {
            let Some(target) = self.song.tracks.get_mut(track) else {
                return;
            };
            target.solo = !target.solo;
            (
                target.name.clone(),
                target.volume,
                target.pan,
                target.muted,
                target.solo,
            )
        };
        self.send_mix(track, volume, pan, muted, solo);
        self.status = format!("{name} {} solo", if solo { "soloed" } else { "unsoloed" });
    }

    fn nudge_octave(&mut self, delta: i32) {
        self.octave = (self.octave + delta).clamp(-4, 4);
        self.input.keymap_mut().set_octave(self.octave);
        self.status = format!("keyboard octave {:+}", self.octave);
    }

    // -- playing -----------------------------------------------------------

    fn play_note(&mut self, pitch: u8, velocity: u8) {
        let track = self.cursor.track;
        self.send(RtCommand::NoteOn {
            track: track as u8,
            pitch,
            velocity,
        });
        self.held.push(HeldNote {
            track: track as u8,
            pitch,
            deadline: Instant::now() + Duration::from_millis(HELD_NOTE_MS),
        });
    }

    fn release_note(&mut self, pitch: u8) {
        let mut index = 0;
        while index < self.held.len() {
            if self.held[index].pitch == pitch {
                // The cursor may have moved since the key went down, so the
                // note-off goes to the track that actually took the note-on.
                let note = self.held.swap_remove(index);
                self.send(RtCommand::NoteOff {
                    track: note.track,
                    pitch,
                });
            } else {
                index += 1;
            }
        }
    }

    fn release_expired_notes(&mut self) {
        let now = Instant::now();
        let mut index = 0;
        while index < self.held.len() {
            if self.held[index].deadline <= now {
                let note = self.held.swap_remove(index);
                self.send(RtCommand::NoteOff {
                    track: note.track,
                    pitch: note.pitch,
                });
            } else {
                index += 1;
            }
        }
    }

    // -- helpers -----------------------------------------------------------

    fn send(&mut self, command: RtCommand) {
        if !self.engine.send(command) {
            // Queue saturated: recover by re-sending the whole song next frame.
            self.resync_pending = true;
        }
    }

    fn send_mix(&mut self, track: usize, volume: f32, pan: f32, muted: bool, solo: bool) {
        self.send(RtCommand::SetTrackMix {
            track: track as u8,
            volume,
            pan,
            muted,
            solo,
        });
    }

    fn set_text_mode(&mut self, on: bool) {
        self.input.keymap_mut().set_text_mode(on);
    }

    /// Blocks the cursor may point at: one per block, plus the `+` slot.
    fn block_slots(&self) -> usize {
        self.song
            .tracks
            .get(self.cursor.track)
            .map_or(1, |track| track.blocks.len() + 1)
    }

    fn clamp_block(&mut self) {
        self.cursor.block = self.cursor.block.min(self.block_slots() - 1);
    }

    fn frames_per_beat(&self) -> u64 {
        Tempo::new(self.song.bpm()).frames_per_beat(self.engine.sample_rate())
    }

    /// Build an immutable snapshot for rendering.
    fn ui_state(&self) -> UiState {
        let shared = self.engine.shared();
        let position_frames = shared.position_frames.load(Ordering::Relaxed);

        let tracks = self
            .song
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| TrackView {
                name: track.name.clone(),
                kind: track.kind,
                preset: track
                    .instrument
                    .preset_index()
                    .map(|i| Instrument::PRESETS[i].to_string())
                    .unwrap_or_else(|| "custom".into()),
                waveform: track.instrument.waveform.name().to_string(),
                octave: track.instrument.octave,
                attack: track.instrument.attack,
                decay: track.instrument.decay,
                sustain: track.instrument.sustain,
                release: track.instrument.release,
                volume: track.volume,
                pan: track.pan,
                muted: track.muted,
                solo: track.solo,
                level: self.meters.level(index),
                blocks: track.blocks.iter().map(|b| b.name.clone()).collect(),
                add_slot: index == self.cursor.track && self.cursor.block >= track.blocks.len(),
                color_index: index as u8,
            })
            .collect();

        UiState {
            bpm: self.song.bpm(),
            playing: self.engine.is_playing(),
            looping: shared.looping.load(Ordering::Relaxed),
            transport_position_beats: position_frames as f32 / self.frames_per_beat() as f32,
            cursor: self.cursor,
            focus: self.focus,
            octave: self.octave,
            tracks,
            status: self.status.clone(),
            backend: self.engine.backend_name().to_string(),
            backend_note: self.engine.note().to_string(),
            sample_rate: self.engine.sample_rate(),
            show_help: self.show_help,
        }
    }
}

// -- terminal lifecycle ----------------------------------------------------

type Backend = CrosstermBackend<io::Stdout>;

fn setup_terminal() -> Result<Terminal<Backend>> {
    enable_raw_mode().context("enabling raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("entering the alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("creating the terminal")
}

/// Restores the terminal on drop, including when the app panics.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headless_app() -> App {
        App::new_headless().expect("headless app should build")
    }

    #[test]
    fn headless_app_starts_with_four_empty_tracks() {
        let app = headless_app();
        assert_eq!(app.song.tracks.len(), 4);
        assert!(app.song.tracks.iter().all(|t| t.blocks.is_empty()));
        assert!(app.ui_state().render().contains("zboub69"));
    }

    #[test]
    fn adding_a_block_appends_it_and_moves_the_cursor() {
        let mut app = headless_app();
        app.cursor = Cursor { track: 0, block: 0 };
        app.handle(Event::AddBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(app.cursor.block, 0);

        app.cursor.block = 1;
        app.handle(Event::AddBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 2);
        assert_eq!(app.cursor.block, 1);
    }

    #[test]
    fn deleting_removes_the_block_under_the_cursor() {
        let mut app = headless_app();
        for _ in 0..2 {
            app.handle(Event::AddBlock);
        }
        app.cursor.block = 0;
        app.handle(Event::DeleteBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(
            app.song.tracks[0].blocks[0].name, "2",
            "the block under the cursor goes, not the last one"
        );
        assert_eq!(app.cursor.block, 0);
    }

    #[test]
    fn deleting_the_last_block_lands_the_cursor_on_the_add_slot() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::DeleteBlock);
        assert!(app.song.tracks[0].blocks.is_empty());
        assert_eq!(app.cursor.block, 0);
    }

    #[test]
    fn deleting_with_the_cursor_on_the_add_slot_changes_nothing() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 1;
        app.handle(Event::DeleteBlock);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
    }

    #[test]
    fn a_deletion_asks_for_a_resync() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::DeleteBlock);
        assert!(
            app.resync_pending,
            "removing a block must rebuild the audio-side mirror"
        );
    }

    #[test]
    fn a_track_refuses_to_grow_past_its_block_ceiling() {
        let mut app = headless_app();
        for _ in 0..MAX_BLOCKS {
            app.handle(Event::AddBlock);
        }
        assert_eq!(app.song.tracks[0].blocks.len(), MAX_BLOCKS);

        app.handle(Event::AddBlock);
        assert_eq!(
            app.song.tracks[0].blocks.len(),
            MAX_BLOCKS,
            "the song must not outgrow the audio-side mirror"
        );
        assert!(app.status.contains("blocks"), "status: {}", app.status);
    }

    #[test]
    fn a_note_is_released_on_the_track_that_played_it() {
        let mut app = headless_app();
        app.cursor = Cursor { track: 0, block: 0 };
        app.handle(Event::NoteOn {
            pitch: 60,
            velocity: 100,
        });
        app.handle(Event::MoveTrack(1));
        app.handle(Event::NoteOff { pitch: 60 });
        assert!(
            app.held.is_empty(),
            "moving the cursor must not strand the held note"
        );
    }

    #[test]
    fn renaming_a_block_replaces_its_name() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.cursor.block = 0;
        app.handle(Event::RenameBlock);
        assert!(app.rename.is_some());
        for character in "Verse".chars() {
            app.handle(Event::TextInput(character));
        }
        app.handle(Event::RenameBlock);
        assert_eq!(app.song.tracks[0].blocks[0].name, "Verse");
        assert!(app.rename.is_none());
    }

    #[test]
    fn cancelling_a_rename_keeps_the_old_name() {
        let mut app = headless_app();
        app.handle(Event::AddBlock);
        app.handle(Event::RenameBlock);
        app.handle(Event::TextInput('X'));
        app.handle(Event::CancelRename);
        assert_eq!(app.song.tracks[0].blocks[0].name, "1");
    }

    #[test]
    fn cursor_stays_in_range_at_the_edges() {
        let mut app = headless_app();
        app.handle(Event::MoveBlock(9));
        assert_eq!(app.cursor.block, 0, "one block slot past an empty track");
        app.handle(Event::MoveTrack(-1));
        assert_eq!(app.cursor.track, 0);
        app.handle(Event::MoveTrack(9));
        assert_eq!(app.cursor.track, app.song.tracks.len() - 1);
    }

    #[test]
    fn cycling_the_track_kind_flips_midi_and_audio() {
        let mut app = headless_app();
        let before = app.song.tracks[0].kind;
        app.handle(Event::CycleTrackKind);
        assert_ne!(app.song.tracks[0].kind, before);
    }

    #[test]
    fn quits_on_the_quit_event() {
        let mut app = headless_app();
        app.handle(Event::Quit);
        assert!(!app.running);
    }
}
