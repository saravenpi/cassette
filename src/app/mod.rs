//! The application: intent handling and the snapshot it publishes.
//!
//! The app drains intent into the song and the engine, then publishes a
//! [`UiState`] snapshot. The Tauri shell drives it: one command per intent, one
//! tick per frame. All timing and synthesis live in [`crate::core`].
//!
//! Work splits across sibling modules, each an `impl App` block: the timeline
//! ([`timeline`]), track state ([`tracks`]), audio samples ([`samples`]) and the
//! transport ([`transport`]).

mod export;
mod samples;
mod timeline;
mod tracks;
mod transport;

pub use samples::{WAVEFORM_BUCKETS, peaks};

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use crossbeam::channel::Receiver;
use rtrb::Producer;

use crate::core::engine::AudioEngine;
use crate::core::meter::MeterBank;
use crate::core::render::RtCommand;
use crate::core::sample::SampleMsg;
use crate::events::Event;
use crate::events::bus::EventBus;
use crate::models::{Instrument, Song};
use crate::ui::view::{BlockView, Cursor, TrackView, UiState};

use tracks::VoiceEdit;
use transport::HeldNote;

/// Beats in one bar, so the ruler and the grid agree.
const BEATS_PER_BAR: f32 = 4.0;
/// Shortest ruler the app publishes, in beats: the grid always has room.
const MIN_RULER_BEATS: f32 = 64.0;
/// Track palette slots the frontend offers.
const COLOR_SLOTS: usize = 8;

/// The running application.
pub struct App {
    song: Song,
    engine: AudioEngine,
    bus: EventBus,
    events: Receiver<Event>,
    cursor: Cursor,
    meters: MeterBank,
    held: Vec<HeldNote>,
    octave: i32,
    /// Buffer of the block-name prompt, when one is open.
    rename: Option<String>,
    status: String,
    resync_pending: bool,
    /// Slot each loaded source file occupies, so one file dropped twice shares one.
    samples: HashMap<String, u8>,
    loaded_samples: HashMap<u8, std::sync::Arc<[f32]>>,
    producer: Option<Producer<SampleMsg>>,
}

impl App {
    pub fn new() -> Result<Self> {
        let song = Song::new();
        let engine = AudioEngine::start(&song).context("starting the audio engine")?;
        Ok(Self::with(song, engine))
    }

    pub fn new_streaming(port: u16) -> Result<Self> {
        let song = Song::new();
        let engine =
            AudioEngine::start_streaming(&song, port).context("starting the streaming engine")?;
        Ok(Self::with(song, engine))
    }

    pub fn new_silent() -> Result<Self> {
        let song = Song::new();
        let engine = AudioEngine::start_silent(&song).context("starting the silent engine")?;
        Ok(Self::with(song, engine))
    }

    fn with(song: Song, mut engine: AudioEngine) -> Self {
        let mut bus = EventBus::new();
        let events = bus.subscribe();
        let meters = MeterBank::new(song.tracks.len());
        let producer = engine.take_sample_producer();
        let status = format!(
            "audio {} at {} Hz ({})",
            engine.backend_name(),
            engine.sample_rate(),
            engine.note()
        );
        Self {
            song,
            engine,
            bus,
            events,
            cursor: Cursor::default(),
            meters,
            held: Vec::new(),
            octave: 0,
            rename: None,
            status,
            resync_pending: false,
            samples: HashMap::new(),
            loaded_samples: HashMap::new(),
            producer,
        }
    }

    /// Apply one intent now. This is the path a Tauri command takes, so a note
    /// sounds on the frame it arrives rather than the next one.
    pub fn submit(&mut self, event: Event) {
        self.bus.send(event);
        self.drain();
    }

    /// Apply everything adapters have queued since the last drain.
    pub fn drain(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.handle(event);
        }
    }

    /// Advance one frame: apply queued intent, resync if the engine refused a
    /// command, expire held notes and read the meters. Call it at display rate.
    pub fn tick(&mut self) {
        self.drain();
        if self.resync_pending {
            self.resync_pending = !self.engine.resync(&self.song);
            if !self.resync_pending {
                self.repush_lanes();
            }
        }
        self.release_expired_notes();
        let peaks = self.engine.shared().peaks();
        self.meters.update(&peaks);
    }

    /// Route one intent to the module that owns it.
    fn handle(&mut self, event: Event) {
        match event {
            Event::TogglePlay => self.toggle_play(),
            Event::Stop => self.stop(),
            Event::Seek { beats } => self.seek(beats),
            Event::ToggleLoop => self.toggle_loop(),
            Event::MoveTrack(delta) => self.move_track(delta),
            Event::MoveBlock(delta) => self.move_block(delta),
            Event::SelectTrack { track } => self.select_track(track),
            Event::SelectBlock { track, block } => self.select_block(track, block),
            Event::CreateBlock {
                track,
                start_beats,
                length_beats,
            } => self.create_block(track, start_beats, length_beats),
            Event::PlaceBlock {
                track,
                block,
                target_track,
                start_beats,
            } => self.place_block(track, block, target_track, start_beats),
            Event::ResizeBlock {
                track,
                block,
                length_beats,
            } => self.resize_block(track, block, length_beats),
            Event::DeleteBlockAt { track, block } => self.delete_block_at(track, block),
            Event::AddNote { .. }
            | Event::MoveNote { .. }
            | Event::ResizeNote { .. }
            | Event::SetNoteVelocity { .. }
            | Event::DeleteNote { .. } => {}
            Event::AddBlock => self.add_block(),
            Event::DeleteBlock => self.delete_block(),
            Event::ImportSample {
                track,
                path,
                start_beats,
            } => self.import_sample(track, path, start_beats),
            Event::CycleTrackKind => self.cycle_track_kind(),
            Event::RenameBlock => self.rename_block(),
            Event::TextInput(character) => self.type_character(character),
            Event::TextBackspace => self.backspace(),
            Event::CancelRename => self.cancel_rename(),
            Event::VolumeDelta(delta) => self.nudge_volume(delta),
            Event::PanDelta(delta) => self.nudge_pan(delta),
            Event::SetTrackMix {
                track,
                volume,
                pan,
                muted,
                solo,
            } => self.set_track_mix(track, volume, pan, muted, solo),
            Event::BpmDelta(delta) => self.nudge_bpm(delta),
            Event::ToggleMute => self.toggle_mute(),
            Event::ToggleSolo => self.toggle_solo(),
            Event::SetPreset { track, preset } => self.set_preset(track, &preset),
            Event::SetInstrument {
                track,
                gain,
                octave,
                attack,
                decay,
                sustain,
                release,
            } => self.set_instrument(
                track,
                VoiceEdit {
                    gain,
                    octave,
                    attack,
                    decay,
                    sustain,
                    release,
                },
            ),
            Event::OctaveDelta(delta) => self.nudge_octave(delta),
            Event::NoteOn { pitch, velocity } => self.play_note(pitch, velocity),
            Event::NoteOff { pitch } => self.release_note(pitch),
            Event::OscControl { address, value } => {
                self.status = format!("osc {address} = {value:.2}");
            }
            Event::ExportAudio { path } => self.export_audio(path),
            Event::RecordAudio { path } => self.record_audio(path),
            Event::ToggleRecord => self.record_audio(None),
            Event::ExportProject { path } => self.export_project(path),
            Event::LoadProject { path } => self.load_project(path),
        }
    }

    /// Push a command to the audio thread, falling back to a full resync when
    /// the queue refuses it.
    fn send(&mut self, command: RtCommand) {
        if !self.engine.send(command) {
            self.resync_pending = true;
        }
    }

    /// Push a whole mixer strip for one track.
    fn send_mix(&mut self, track: usize, volume: f32, pan: f32, muted: bool, solo: bool) {
        self.send(RtCommand::SetTrackMix {
            track: track as u8,
            volume,
            pan,
            muted,
            solo,
        });
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

    /// Build an immutable snapshot for the webview.
    pub fn ui_state(&self) -> UiState {
        let shared = self.engine.shared();
        let position_frames = shared.position_frames.load(Ordering::Relaxed);

        let tracks: Vec<TrackView> = self
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
                    .unwrap_or_else(|| "Custom".into()),
                waveform: track.instrument.waveform.name().to_string(),
                gain: track.instrument.gain,
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
                blocks: track
                    .blocks
                    .iter()
                    .enumerate()
                    .map(|(block, entry)| BlockView {
                        name: entry.name.clone(),
                        kind: entry.kind,
                        start_beats: entry.start_beats,
                        length_beats: entry.length_beats,
                        source: entry.source.clone(),
                        selected: index == self.cursor.track && block == self.cursor.block,
                        notes: entry.notes.clone(),
                    })
                    .collect(),
                color_index: (index % COLOR_SLOTS) as u8,
            })
            .collect();

        let length_beats = ruler_length(&tracks);

        UiState {
            bpm: self.song.bpm(),
            playing: self.engine.is_playing(),
            looping: shared.looping.load(Ordering::Relaxed),
            transport_position_beats: (position_frames as f64 / self.frames_per_beat()) as f32,
            length_beats,
            bars: (length_beats / BEATS_PER_BAR).ceil() as u32,
            beats_per_bar: BEATS_PER_BAR as u32,
            cursor: self.cursor,
            octave: self.octave,
            presets: Instrument::PRESETS
                .iter()
                .map(|name| name.to_string())
                .collect(),
            tracks,
            status: self.status.clone(),
            renaming: self.rename.is_some(),
            backend: self.engine.backend_name().to_string(),
            backend_note: self.engine.note().to_string(),
            sample_rate: self.engine.sample_rate(),
        }
    }
}

/// How far the ruler reaches: the last block end rounded up to a bar, never less
/// than [`MIN_RULER_BEATS`].
fn ruler_length(tracks: &[TrackView]) -> f32 {
    let end = tracks
        .iter()
        .map(TrackView::end_beats)
        .fold(0.0f32, f32::max);
    ((end / BEATS_PER_BAR).ceil() * BEATS_PER_BAR).max(MIN_RULER_BEATS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    #[test]
    fn silent_app_starts_with_four_empty_tracks() {
        let app = headless_app();
        assert_eq!(app.song.tracks.len(), 4);
        assert!(app.song.tracks.iter().all(|t| t.blocks.is_empty()));
        assert_eq!(app.ui_state().backend, "silent");
    }

    #[test]
    fn submit_applies_intent_without_waiting_for_a_tick() {
        let mut app = headless_app();
        app.submit(Event::AddBlock);
        assert_eq!(
            app.song.tracks[0].blocks.len(),
            1,
            "a command must land on the frame it arrives, not the next"
        );
    }

    #[test]
    fn the_ruler_always_has_room_for_four_bars() {
        let state = headless_app().ui_state();
        assert_eq!(state.length_beats, 64.0);
        assert_eq!(state.bars, 16);
        assert_eq!(state.beats_per_bar, 4);
        assert_eq!(state.presets.len(), Instrument::PRESETS.len());
        assert_eq!(state.presets[0], "Lead");
    }

    #[test]
    fn the_ruler_grows_to_hold_the_last_block() {
        let mut app = headless_app();
        app.submit(Event::CreateBlock {
            track: 0,
            start_beats: 100.0,
            length_beats: 8.0,
        });
        let state = app.ui_state();
        assert_eq!(state.length_beats, 108.0);
        assert_eq!(state.bars, 27);
    }

    #[test]
    fn only_the_block_under_the_cursor_is_selected() {
        let mut app = headless_app();
        for start in [0.0, 8.0] {
            app.submit(Event::CreateBlock {
                track: 0,
                start_beats: start,
                length_beats: 4.0,
            });
        }
        app.submit(Event::SelectBlock { track: 0, block: 0 });
        let state = app.ui_state();
        assert!(state.tracks[0].blocks[0].selected);
        assert!(!state.tracks[0].blocks[1].selected);
        assert_eq!(state.cursor, Cursor { track: 0, block: 0 });
    }

    #[test]
    fn a_track_view_names_its_voice_waveform_and_colour() {
        let state = headless_app().ui_state();
        assert_eq!(state.tracks[0].preset, "Lead");
        assert_eq!(state.tracks[2].preset, "Pluck");
        assert_eq!(state.tracks[0].waveform, "Square");
        assert_eq!(state.tracks[0].color_index, 0);
        assert_eq!(state.tracks[3].color_index, 3);
    }
}
