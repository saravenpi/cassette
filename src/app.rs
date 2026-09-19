//! The application: terminal lifecycle, the event loop and command handling.
//!
//! The loop is deliberately small — poll input, drain the event bus, apply
//! intent to the song and the engine, publish a [`UiState`] snapshot, draw.
//! All timing and synthesis live in [`crate::core`].

use std::io;
use std::path::PathBuf;
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

use crate::core::engine::AudioEngine;
use crate::core::offline;
use crate::core::recorder::{MeterBank, PerformanceRecorder};
use crate::core::render::RtCommand;
use crate::core::sequencer::StepData;
use crate::core::MAX_STEPS;
use crate::events::Event;
use crate::events::bus::EventBus;
use crate::input::InputHandler;
use crate::models::{Instrument, Song};
use crate::ui::layout::Pane;
use crate::ui::renderer;
use crate::ui::view::{Cursor, StepView, TrackView, UiState, View};
use crate::utils::frequency::note_name;

/// How long a previewed step or played note rings before auto-release.
const PREVIEW_MS: u64 = 220;
/// How long a key-held note rings when the terminal cannot report releases.
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
    recorder: PerformanceRecorder,
    held: Vec<HeldNote>,
    octave: i32,
    status: String,
    show_help: bool,
    running: bool,
    resync_pending: bool,
}

impl App {
    /// Build an app with the live audio backend.
    pub fn new() -> Result<Self> {
        let song = Song::demo();
        let engine = AudioEngine::start(&song).context("starting the audio engine")?;
        Ok(Self::with(song, engine))
    }

    /// Build an app that streams audio over loopback TCP (audio over SSH).
    pub fn new_streaming(port: u16) -> Result<Self> {
        let song = Song::demo();
        let engine =
            AudioEngine::start_streaming(&song, port).context("starting the streaming engine")?;
        Ok(Self::with(song, engine))
    }

    /// Build an app whose engine makes no sound (headless / CI).
    pub fn new_headless() -> Result<Self> {
        let song = Song::demo();
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
            focus: Pane::Pattern,
            meters,
            recorder: PerformanceRecorder::new(),
            held: Vec::new(),
            octave: 0,
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
            Event::ToggleRecord => self.toggle_record(),
            Event::ToggleLoop => {
                let looping = !self.engine.shared().looping.load(Ordering::Relaxed);
                self.send(RtCommand::SetLooping { looping });
                self.status = format!("looping {}", if looping { "on" } else { "off" });
            }
            Event::ToggleHelp => self.show_help = !self.show_help,
            Event::CycleFocus => self.focus = self.focus.next(),
            Event::MoveCursor { dx, dy } => self.move_cursor(dx, dy),
            Event::ToggleStep => self.toggle_step(),
            Event::PitchDelta(delta) => self.nudge_pitch(delta),
            Event::VelocityDelta(delta) => self.nudge_velocity(delta),
            Event::VolumeDelta(delta) => self.nudge_volume(delta),
            Event::PanDelta(delta) => self.nudge_pan(delta),
            Event::BpmDelta(delta) => self.nudge_bpm(delta),
            Event::ToggleMute => self.toggle_mute(),
            Event::ToggleSolo => self.toggle_solo(),
            Event::CycleWaveform => self.cycle_waveform(),
            Event::CyclePreset => self.cycle_preset(),
            Event::OctaveDelta(delta) => self.nudge_octave(delta),
            Event::LengthDelta(delta) => self.nudge_length(delta),
            Event::NoteOn { pitch, velocity } => self.play_note(pitch, velocity),
            Event::NoteOff { pitch } => self.release_note(pitch),
            Event::Export => self.export(),
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

    fn toggle_record(&mut self) {
        if self.recorder.toggle() {
            self.status = "recording armed — play notes to capture them".into();
            if !self.engine.is_playing() {
                self.send(RtCommand::Play);
            }
        } else {
            self.status = "recording off".into();
        }
    }

    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let tracks = self.song.tracks.len().max(1) as i32;
        let steps = self.song.length_steps.max(1) as i32;
        self.cursor.track = (self.cursor.track as i32 + dy).rem_euclid(tracks) as usize;
        self.cursor.step = (self.cursor.step as i32 + dx).rem_euclid(steps) as usize;
    }

    fn toggle_step(&mut self) {
        let (track, step) = (self.cursor.track, self.cursor.step);
        self.song.toggle_step(track, step);
        let data = StepData::from_step(self.song.step(track, step));
        self.send(RtCommand::SetStep {
            track: track as u8,
            step: step as u8,
            data,
        });
        if data.active {
            self.preview(track, data.pitch, data.velocity);
            self.status = format!("step {} on ({})", step + 1, note_name(data.pitch));
        } else {
            self.status = format!("step {} off", step + 1);
        }
    }

    fn nudge_pitch(&mut self, delta: i32) {
        let (track, step) = (self.cursor.track, self.cursor.step);
        let mut value = self.song.step(track, step);
        value.pitch = (value.pitch as i32 + delta).clamp(0, 127) as u8;
        self.song.set_step(track, step, value);
        self.send(RtCommand::SetStep {
            track: track as u8,
            step: step as u8,
            data: StepData::from_step(value),
        });
        self.status = format!("step {} → {}", step + 1, note_name(value.pitch));
        if value.active {
            self.preview(track, value.pitch, value.velocity);
        }
    }

    fn nudge_velocity(&mut self, delta: i32) {
        let (track, step) = (self.cursor.track, self.cursor.step);
        let mut value = self.song.step(track, step);
        value.velocity = (value.velocity as i32 + delta).clamp(1, 127) as u8;
        self.song.set_step(track, step, value);
        self.send(RtCommand::SetStep {
            track: track as u8,
            step: step as u8,
            data: StepData::from_step(value),
        });
        self.status = format!("velocity {}", value.velocity);
    }

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
        self.send(RtCommand::SetBpm { bpm: self.song.bpm() });
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

    fn cycle_waveform(&mut self) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get_mut(track) else {
            return;
        };
        target.instrument.waveform = target.instrument.waveform.next();
        let instrument = target.instrument;
        self.send(RtCommand::SetTrackInstrument {
            track: track as u8,
            instrument,
        });
        self.status = format!("waveform {}", instrument.waveform.name());
    }

    fn cycle_preset(&mut self) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get_mut(track) else {
            return;
        };
        let next = target
            .instrument
            .preset_index()
            .map(|index| (index + 1) % Instrument::PRESETS.len())
            .unwrap_or(0);
        target.instrument = Instrument::preset(Instrument::PRESETS[next]);
        let instrument = target.instrument;
        self.send(RtCommand::SetTrackInstrument {
            track: track as u8,
            instrument,
        });
        self.status = format!("preset {}", Instrument::PRESETS[next]);
    }

    fn nudge_octave(&mut self, delta: i32) {
        self.octave = (self.octave + delta).clamp(-4, 4);
        self.input.keymap_mut().set_octave(self.octave);
        self.status = format!("keyboard octave {:+}", self.octave);
    }

    fn nudge_length(&mut self, delta: i32) {
        let length = (self.song.length_steps as i32 + delta).clamp(1, MAX_STEPS as i32) as usize;
        self.song.set_length(length);
        self.cursor.step = self.cursor.step.min(length - 1);
        self.send(RtCommand::SetLength {
            steps: length as u8,
        });
        self.status = format!("pattern length {length} steps");
    }

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

        let step = self.current_step();
        if self.recorder.capture(&mut self.song, track, step, pitch, velocity) {
            let data = StepData::from_step(self.song.step(track, step));
            self.send(RtCommand::SetStep {
                track: track as u8,
                step: step as u8,
                data,
            });
            self.status = format!(
                "recorded {} at step {}",
                note_name(pitch),
                step + 1
            );
        }
    }

    fn release_note(&mut self, pitch: u8) {
        let track = self.cursor.track as u8;
        self.send(RtCommand::NoteOff { track, pitch });
        self.held
            .retain(|note| !(note.pitch == pitch && note.track == track));
    }

    fn preview(&mut self, track: usize, pitch: u8, velocity: u8) {
        self.send(RtCommand::NoteOn {
            track: track as u8,
            pitch,
            velocity,
        });
        self.held.push(HeldNote {
            track: track as u8,
            pitch,
            deadline: Instant::now() + Duration::from_millis(PREVIEW_MS),
        });
    }

    fn export(&mut self) {
        let path = PathBuf::from("zboub69-mix.wav");
        match offline::export_song(&self.song, &path, 2, self.engine.sample_rate()) {
            Ok(()) => self.status = format!("exported {} (2 loops)", path.display()),
            Err(error) => self.status = format!("export failed: {error}"),
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

    fn current_step(&self) -> usize {
        self.song.current_step(
            self.engine.shared().position_frames.load(Ordering::Relaxed),
            self.engine.sample_rate(),
        )
    }

    /// Build an immutable snapshot for rendering.
    fn ui_state(&self) -> UiState {
        let shared = self.engine.shared();
        let position_step = self.song.current_step(
            shared.position_frames.load(Ordering::Relaxed),
            self.engine.sample_rate(),
        );

        let tracks = self
            .song
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| TrackView {
                name: track.name.clone(),
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
                peak: self.meters.peak(index),
                steps: track
                    .steps
                    .iter()
                    .map(|step| StepView {
                        active: step.active,
                        pitch: step.pitch,
                    })
                    .collect(),
                color_index: index as u8,
            })
            .collect();

        UiState {
            bpm: self.song.bpm(),
            playing: self.engine.is_playing(),
            recording: self.recorder.is_armed(),
            looping: shared.looping.load(Ordering::Relaxed),
            position_step,
            length_steps: self.song.length_steps,
            cursor: self.cursor,
            focus: self.focus,
            octave: self.octave,
            tracks,
            status: self.status.clone(),
            backend: self.engine.backend_name().to_string(),
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
    fn headless_app_starts_with_the_demo_song() {
        let app = headless_app();
        assert_eq!(app.song.tracks.len(), 6);
        assert!(app.ui_state().render().contains("zboub69"));
    }

    #[test]
    fn toggling_a_step_updates_the_song_and_cursor_stays_in_range() {
        let mut app = headless_app();
        app.cursor = Cursor { step: 0, track: 0 };
        let before = app.song.step(0, 0).active;
        app.toggle_step();
        assert_ne!(app.song.step(0, 0).active, before);
    }

    #[test]
    fn cursor_wraps_around_edges() {
        let mut app = headless_app();
        app.cursor = Cursor { step: 0, track: 0 };
        app.move_cursor(-1, -1);
        assert_eq!(app.cursor.track, app.song.tracks.len() - 1);
        assert_eq!(app.cursor.step, app.song.length_steps - 1);
    }

    #[test]
    fn changing_length_keeps_the_cursor_valid() {
        let mut app = headless_app();
        app.cursor.step = 15;
        app.nudge_length(-8);
        assert_eq!(app.song.length_steps, 8);
        assert!(app.cursor.step < 8);
    }

    #[test]
    fn preset_cycling_changes_the_instrument() {
        let mut app = headless_app();
        app.cursor.track = 0;
        let before = app.song.tracks[0].instrument;
        app.cycle_preset();
        assert_ne!(app.song.tracks[0].instrument, before);
    }

    #[test]
    fn quits_on_the_quit_event() {
        let mut app = headless_app();
        app.handle(Event::Quit);
        assert!(!app.running);
    }
}
