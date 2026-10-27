//! Track state: selection, kind, mixer strip and the track's voice.
//!
//! Every edit here lands on the song first and reaches the audio thread second,
//! through the same `RtCommand` a resync would use.

use super::App;
use crate::app::effects::TrackShift;
use crate::core::MAX_TRACKS;
use crate::core::meter::MeterBank;
use crate::core::render::RtCommand;
use crate::models::{Instrument, Track, TrackKind};

/// The voice fields a `SetInstrument` intent carries.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VoiceEdit {
    /// Per-voice output gain, `[0, 1]`.
    pub gain: f32,
    /// Octave transposition applied to played notes.
    pub octave: i32,
    /// Attack time in seconds.
    pub attack: f32,
    /// Decay time in seconds.
    pub decay: f32,
    /// Sustain level, `[0, 1]`.
    pub sustain: f32,
    /// Release time in seconds.
    pub release: f32,
}

impl App {
    pub(crate) fn add_track(&mut self, kind: TrackKind) {
        if self.song.tracks.len() >= MAX_TRACKS {
            self.status = format!("the song already holds {MAX_TRACKS} tracks");
            return;
        }
        let prefix = match kind {
            TrackKind::Midi => "MIDI",
            TrackKind::Audio => "Audio",
        };
        let mut n = 1;
        loop {
            let candidate = format!("{prefix} {n}");
            if !self
                .song
                .tracks
                .iter()
                .any(|t| t.name.eq_ignore_ascii_case(&candidate))
            {
                break;
            }
            n += 1;
        }
        let name = format!("{prefix} {n}");
        let color_index = (self.song.tracks.len() % 8) as u8;
        let new_track =
            Track::new(name.clone(), kind, kind.default_preset()).with_color(color_index);
        self.song.tracks.push(new_track);
        self.cursor.track = self.song.tracks.len() - 1;
        self.clamp_block();
        self.engine.resync(&self.song);
        self.resync();
        self.meters = MeterBank::new(self.song.tracks.len());
        for t in 0..self.song.tracks.len() {
            self.touch_notes(t);
        }
        self.status = format!("added track {name}");
    }

    pub(crate) fn delete_track(&mut self, track: usize) {
        if self.song.tracks.len() <= 1 {
            self.status = "cannot delete the last track".into();
            return;
        }
        if track >= self.song.tracks.len() {
            self.status = "no track there to delete".into();
            return;
        }
        let removed = self.song.tracks.remove(track);
        self.cursor.track = self.cursor.track.min(self.song.tracks.len() - 1);
        self.clamp_block();
        self.garbage_collect_samples();
        self.remap_sidechains(TrackShift::Deleted(track));
        if !self.engine.delete_track(track) {
            self.resync_pending = true;
        }
        for i in track..(MAX_TRACKS - 1) {
            self.note_revisions[i] = self.note_revisions[i + 1];
        }
        self.note_revisions[MAX_TRACKS - 1] = 0;
        self.meters = MeterBank::new(self.song.tracks.len());
        for t in 0..self.song.tracks.len() {
            self.touch_notes(t);
            self.push_effects(t);
        }
        self.status = format!("deleted track {}", removed.name);
    }

    pub(crate) fn duplicate_track(&mut self, track: usize) {
        if track >= self.song.tracks.len() {
            self.status = "no track there to duplicate".into();
            return;
        }
        if self.song.tracks.len() >= MAX_TRACKS {
            self.status = format!("the song already holds {MAX_TRACKS} tracks");
            return;
        }
        let mut copy = self.song.tracks[track].clone();
        copy.name = format!("{} copy", copy.name);
        for effect in &mut copy.effects {
            effect.id = self.song.next_effect_id();
        }
        let insert_index = track + 1;
        copy.color_index = Some((insert_index % 8) as u8);
        self.song.tracks.insert(insert_index, copy);
        self.cursor.track = insert_index;
        self.clamp_block();
        self.remap_sidechains(TrackShift::Inserted(insert_index));
        self.engine.resync(&self.song);
        self.resync();
        self.meters = MeterBank::new(self.song.tracks.len());
        for t in 0..self.song.tracks.len() {
            self.touch_notes(t);
        }
        self.status = format!("duplicated track {}", self.song.tracks[insert_index].name);
    }

    pub(crate) fn reorder_track(&mut self, from: usize, to: usize) {
        let count = self.song.tracks.len();
        if from >= count || to >= count || from == to {
            return;
        }
        let track_name = self.song.tracks[from].name.clone();
        if from < to {
            self.song.tracks[from..=to].rotate_left(1);
            self.note_revisions[from..=to].rotate_left(1);
        } else {
            self.song.tracks[to..=from].rotate_right(1);
            self.note_revisions[to..=from].rotate_right(1);
        }
        self.remap_sidechains(TrackShift::Moved { from, to });
        if self.cursor.track == from {
            self.cursor.track = to;
        } else if from < to && self.cursor.track > from && self.cursor.track <= to {
            self.cursor.track -= 1;
        } else if to < from && self.cursor.track >= to && self.cursor.track < from {
            self.cursor.track += 1;
        }
        self.clamp_block();
        if !self.engine.move_track(from, to) {
            self.resync_pending = true;
        }
        for t in 0..count {
            self.touch_notes(t);
            self.push_effects(t);
        }
        self.status = format!("moved track {track_name} to position {}", to + 1);
    }

    pub(crate) fn set_track_color(&mut self, track: usize, color_index: u8) {
        if let Some(target) = self.song.tracks.get_mut(track) {
            target.color_index = Some(color_index % 8);
            self.status = format!("changed color for track {}", target.name);
        }
    }

    pub(crate) fn rename_track(&mut self, track: usize, name: &str) {
        if track >= self.song.tracks.len() {
            self.status = "no track there to rename".into();
            return;
        }
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.status = "track name cannot be empty".into();
            return;
        }
        let capped: String = trimmed.chars().take(32).collect();
        self.song.tracks[track].name = capped.clone();
        self.status = format!("track renamed to {capped}");
    }

    pub(crate) fn set_instrument_enabled(&mut self, track: usize, enabled: bool) {
        let Some(target) = self.song.tracks.get_mut(track) else {
            self.status = "no track there to edit".into();
            return;
        };
        target.instrument.enabled = enabled;
        let instrument = target.instrument;
        self.send(RtCommand::SetTrackInstrument {
            track: track as u8,
            instrument,
        });
        self.status = format!(
            "track {} instrument {}",
            track + 1,
            if enabled { "on" } else { "off" }
        );
    }

    pub(crate) fn set_instrument_tone(
        &mut self,
        track: usize,
        cutoff: f32,
        resonance: f32,
        detune: f32,
        sub: f32,
    ) {
        let Some(target) = self.song.tracks.get_mut(track) else {
            self.status = "no track there to edit".into();
            return;
        };
        let cutoff = if cutoff.is_finite() {
            cutoff.clamp(20.0, 20_000.0)
        } else {
            20_000.0
        };
        let resonance = if resonance.is_finite() {
            resonance.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let detune = if detune.is_finite() {
            detune.clamp(0.0, 100.0)
        } else {
            0.0
        };
        let sub = if sub.is_finite() {
            sub.clamp(0.0, 1.0)
        } else {
            0.0
        };
        target.instrument.cutoff = cutoff;
        target.instrument.resonance = resonance;
        target.instrument.detune = detune;
        target.instrument.sub = sub;
        let instrument = target.instrument;
        self.send(RtCommand::SetTrackInstrument {
            track: track as u8,
            instrument,
        });
        self.status = format!(
            "track {} tone cutoff {:.0} Hz res {:.2}",
            track + 1,
            instrument.cutoff,
            instrument.resonance
        );
    }

    /// Move the track cursor by `delta`, clamped to the song.
    pub(crate) fn move_track(&mut self, delta: i32) {
        let tracks = self.song.tracks.len().max(1) as i32;
        self.cursor.track = (self.cursor.track as i32 + delta).clamp(0, tracks - 1) as usize;
        self.clamp_block();
    }

    /// Move the block cursor by `delta`, clamped to the selected track's slots.
    pub(crate) fn move_block(&mut self, delta: i32) {
        let blocks = self.block_slots().max(1) as i32;
        self.cursor.block = (self.cursor.block as i32 + delta).clamp(0, blocks - 1) as usize;
    }

    /// Put the cursor on a track, leaving the block cursor where it was.
    pub(crate) fn select_track(&mut self, track: usize) {
        self.cursor.track = track.min(self.song.tracks.len().saturating_sub(1));
        self.clamp_block();
    }

    /// Put the cursor on one block of one track.
    pub(crate) fn select_block(&mut self, track: usize, block: usize) {
        self.select_track(track);
        self.cursor.block = block.min(self.block_slots() - 1);
    }

    /// Flip the selected track between MIDI and audio. The blocks stay: both
    /// kinds render on either lane.
    pub(crate) fn cycle_track_kind(&mut self) {
        let track_index = self.cursor.track;
        let Some(track) = self.song.tracks.get_mut(track_index) else {
            return;
        };
        track.kind = track.kind.flipped();
        track.instrument = track.kind.default_preset();
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
        self.push_mirror(track_index);
        self.status = format!("track {} is now {}", track_index + 1, kind.name());
    }

    /// Set a track's whole mixer strip at once, which is what a fader drag is.
    pub(crate) fn set_track_mix(
        &mut self,
        track: usize,
        volume: f32,
        pan: f32,
        muted: bool,
        solo: bool,
    ) {
        let Some(target) = self.song.tracks.get_mut(track) else {
            return;
        };
        target.volume = volume.clamp(0.0, 1.0);
        target.pan = pan.clamp(-1.0, 1.0);
        target.muted = muted;
        target.solo = solo;
        let (volume, pan, muted, solo) = (target.volume, target.pan, target.muted, target.solo);
        self.send_mix(track, volume, pan, muted, solo);
        self.status = format!("volume {volume:.2} pan {pan:+.2}");
    }

    /// Nudge the selected track's volume.
    pub(crate) fn nudge_volume(&mut self, delta: f32) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get(track) else {
            return;
        };
        let (volume, pan, muted, solo) =
            (target.volume + delta, target.pan, target.muted, target.solo);
        self.set_track_mix(track, volume, pan, muted, solo);
    }

    /// Nudge the selected track's pan.
    pub(crate) fn nudge_pan(&mut self, delta: f32) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get(track) else {
            return;
        };
        let (volume, pan, muted, solo) =
            (target.volume, target.pan + delta, target.muted, target.solo);
        self.set_track_mix(track, volume, pan, muted, solo);
    }

    /// Toggle mute on the selected track.
    pub(crate) fn toggle_mute(&mut self) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get(track) else {
            return;
        };
        let (name, volume, pan, muted, solo) = (
            target.name.clone(),
            target.volume,
            target.pan,
            !target.muted,
            target.solo,
        );
        self.set_track_mix(track, volume, pan, muted, solo);
        self.status = format!("{name} {}", if muted { "muted" } else { "unmuted" });
    }

    /// Toggle solo on the selected track.
    pub(crate) fn toggle_solo(&mut self) {
        let track = self.cursor.track;
        let Some(target) = self.song.tracks.get(track) else {
            return;
        };
        let (name, volume, pan, muted, solo) = (
            target.name.clone(),
            target.volume,
            target.pan,
            target.muted,
            !target.solo,
        );
        self.set_track_mix(track, volume, pan, muted, solo);
        self.status = format!("{name} {} solo", if solo { "soloed" } else { "unsoloed" });
    }

    /// Replace a track's voice with a named preset, as the VST pane does.
    pub(crate) fn set_preset(&mut self, track: usize, preset: &str) {
        let Some(target) = self.song.tracks.get_mut(track) else {
            return;
        };
        target.instrument = Instrument::preset(preset);
        let instrument = target.instrument;
        self.send(RtCommand::SetTrackInstrument {
            track: track as u8,
            instrument,
        });
        self.push_mirror(track);
        self.status = format!("track {} voice {preset}", track + 1);
    }

    /// Edit a track's voice: keep its waveform, take the given envelope and
    /// transposition.
    pub(crate) fn set_instrument(&mut self, track: usize, edit: VoiceEdit) {
        let Some(target) = self.song.tracks.get_mut(track) else {
            return;
        };
        let waveform = target.instrument.waveform;
        target.instrument = Instrument::new(
            waveform,
            edit.attack,
            edit.decay,
            edit.sustain,
            edit.release,
            edit.gain,
            edit.octave,
        );
        let instrument = target.instrument;
        self.send(RtCommand::SetTrackInstrument {
            track: track as u8,
            instrument,
        });
        self.push_mirror(track);
        self.status = format!("track {} voice edited", track + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Event;
    use crate::models::{TrackKind, Waveform};
    use crate::ui::view::Cursor;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
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
    fn selecting_a_track_keeps_the_block_cursor_in_range() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.handle(Event::SelectTrack { track: 1 });
        assert_eq!(app.cursor.track, 1);
        assert_eq!(app.cursor.block, 0, "an empty lane has only the add slot");
    }

    #[test]
    fn selecting_a_block_selects_its_track() {
        let mut app = headless_app();
        app.handle(Event::SelectBlock { track: 2, block: 0 });
        assert_eq!(app.cursor, Cursor { track: 2, block: 0 });
    }

    #[test]
    fn cycling_the_track_kind_flips_midi_and_audio() {
        let mut app = headless_app();
        let before = app.song.tracks[0].kind;
        app.handle(Event::CycleTrackKind);
        assert_ne!(app.song.tracks[0].kind, before);
        assert_eq!(app.song.tracks[0].instrument, Instrument::preset("Pluck"));
    }

    #[test]
    fn cycling_the_kind_keeps_the_blocks() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 0,
            start_beats: 4.0,
            length_beats: 4.0,
        });
        app.handle(Event::CycleTrackKind);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert_eq!(app.song.tracks[0].blocks[0].kind, TrackKind::Midi);
        assert_eq!(app.song.tracks[0].kind, TrackKind::Audio);
    }

    #[test]
    fn a_fader_drag_sets_the_whole_strip() {
        let mut app = headless_app();
        app.handle(Event::SetTrackMix {
            track: 1,
            volume: 0.25,
            pan: -0.5,
            muted: true,
            solo: true,
        });
        let track = &app.song.tracks[1];
        assert_eq!(track.volume, 0.25);
        assert_eq!(track.pan, -0.5);
        assert!(track.muted && track.solo);
    }

    #[test]
    fn volume_and_pan_deltas_stay_in_range() {
        let mut app = headless_app();
        for _ in 0..20 {
            app.handle(Event::VolumeDelta(0.2));
            app.handle(Event::PanDelta(-0.3));
        }
        assert_eq!(app.song.tracks[0].volume, 1.0);
        assert_eq!(app.song.tracks[0].pan, -1.0);
    }

    #[test]
    fn mute_and_solo_toggle_the_selected_track() {
        let mut app = headless_app();
        app.handle(Event::ToggleMute);
        assert!(app.song.tracks[0].muted);
        app.handle(Event::ToggleSolo);
        assert!(app.song.tracks[0].solo);
        app.handle(Event::ToggleMute);
        assert!(!app.song.tracks[0].muted);
    }

    #[test]
    fn a_preset_replaces_the_voice() {
        let mut app = headless_app();
        app.handle(Event::SetPreset {
            track: 0,
            preset: "Bass".into(),
        });
        assert_eq!(app.song.tracks[0].instrument, Instrument::preset("Bass"));
        assert_eq!(app.ui_state().tracks[0].preset, "Bass");
    }

    #[test]
    fn an_envelope_edit_keeps_the_waveform() {
        let mut app = headless_app();
        app.song.tracks[0].instrument.waveform = Waveform::Saw;
        app.handle(Event::SetInstrument {
            track: 0,
            gain: 0.4,
            octave: -2,
            attack: 0.01,
            decay: 0.2,
            sustain: 0.5,
            release: 0.3,
        });
        let instrument = app.song.tracks[0].instrument;
        assert_eq!(instrument.waveform, Waveform::Saw);
        assert_eq!(instrument.gain, 0.4);
        assert_eq!(instrument.octave, -2);
        assert_eq!(instrument.release, 0.3);
        assert_eq!(app.ui_state().tracks[0].preset, "Custom");
    }

    #[test]
    fn track_management_add_delete_duplicate_and_rename() {
        let mut app = headless_app();
        assert_eq!(app.song.tracks.len(), 4);
        assert_eq!(app.ui_state().tracks.len(), 4);

        app.handle(Event::AddTrack {
            kind: TrackKind::Midi,
        });
        assert_eq!(app.song.tracks.len(), 5);
        assert_eq!(app.song.tracks[4].name, "MIDI 3");
        assert_eq!(app.ui_state().tracks.len(), 5);

        app.handle(Event::Undo);
        assert_eq!(app.song.tracks.len(), 4);
        assert_eq!(app.ui_state().tracks.len(), 4);

        app.handle(Event::Redo);
        assert_eq!(app.song.tracks.len(), 5);
        assert_eq!(app.ui_state().tracks.len(), 5);

        app.handle(Event::DuplicateTrack { track: 4 });
        assert_eq!(app.song.tracks.len(), 6);
        assert_eq!(app.song.tracks[5].name, "MIDI 3 copy");
        assert_eq!(app.ui_state().tracks.len(), 6);

        app.handle(Event::RenameTrack {
            track: 5,
            name: "  Synthesizer lead with a very very long name that exceeds thirty two chars  "
                .into(),
        });
        assert_eq!(app.song.tracks[5].name, "Synthesizer lead with a very ver");
        assert_eq!(app.song.tracks[5].name.len(), 32);

        app.handle(Event::RenameTrack {
            track: 5,
            name: "   ".into(),
        });
        assert_eq!(app.song.tracks[5].name, "Synthesizer lead with a very ver");
        assert_eq!(app.status, "track name cannot be empty");

        app.handle(Event::DeleteTrack { track: 5 });
        assert_eq!(app.song.tracks.len(), 5);
        assert_eq!(app.ui_state().tracks.len(), 5);

        app.handle(Event::Undo);
        assert_eq!(app.song.tracks.len(), 6);
        assert_eq!(app.ui_state().tracks.len(), 6);

        for _ in 0..10 {
            app.handle(Event::AddTrack {
                kind: TrackKind::Audio,
            });
        }
        assert_eq!(app.song.tracks.len(), MAX_TRACKS);
        app.handle(Event::AddTrack {
            kind: TrackKind::Midi,
        });
        assert_eq!(app.song.tracks.len(), MAX_TRACKS);
        assert!(app.status.contains("already holds"));

        while app.song.tracks.len() > 1 {
            app.handle(Event::DeleteTrack { track: 0 });
        }
        assert_eq!(app.song.tracks.len(), 1);
        app.handle(Event::DeleteTrack { track: 0 });
        assert_eq!(app.song.tracks.len(), 1);
        assert_eq!(app.status, "cannot delete the last track");
    }

    #[test]
    fn instrument_bypass_and_tone_controls() {
        let mut app = headless_app();
        app.handle(Event::SetInstrumentEnabled {
            track: 0,
            enabled: false,
        });
        assert!(!app.song.tracks[0].instrument.enabled);
        assert_eq!(app.ui_state().tracks[0].preset, "Lead");
        assert_eq!(app.status, "track 1 instrument off");

        app.handle(Event::SetInstrumentTone {
            track: 0,
            cutoff: 500.0,
            resonance: 0.8,
            detune: 25.0,
            sub: 0.5,
        });
        assert_eq!(app.song.tracks[0].instrument.cutoff, 500.0);
        assert_eq!(app.song.tracks[0].instrument.resonance, 0.8);
        assert_eq!(app.song.tracks[0].instrument.detune, 25.0);
        assert_eq!(app.song.tracks[0].instrument.sub, 0.5);

        app.handle(Event::SetInstrumentTone {
            track: 0,
            cutoff: 100_000.0,
            resonance: 5.0,
            detune: 200.0,
            sub: 3.0,
        });
        assert_eq!(app.song.tracks[0].instrument.cutoff, 20_000.0);
        assert_eq!(app.song.tracks[0].instrument.resonance, 1.0);
        assert_eq!(app.song.tracks[0].instrument.detune, 100.0);
        assert_eq!(app.song.tracks[0].instrument.sub, 1.0);

        app.handle(Event::Undo);
        assert_eq!(app.song.tracks[0].instrument.cutoff, 20_000.0);
    }

    #[test]
    fn deleting_an_empty_track_while_playing_preserves_playback() {
        let mut app = headless_app();
        app.handle(Event::CreateBlock {
            track: 1,
            start_beats: 0.0,
            length_beats: 16.0,
        });
        app.handle(Event::AddNote {
            track: 1,
            block: 0,
            pitch: 60,
            start_beats: 0.0,
            length_beats: 16.0,
            velocity: 100,
        });
        app.handle(Event::TogglePlay);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut peak = 0.0f32;
        while peak <= 0.01 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            peak = app.engine.shared().peaks()[1];
        }
        assert!(peak > 0.01, "track 1 should be sounding");
        app.handle(Event::DeleteTrack { track: 0 });
        std::thread::sleep(std::time::Duration::from_millis(50));
        let peak_after = app.engine.shared().peaks()[0];
        assert!(
            peak_after > 0.01,
            "track 0 (shifted from track 1) should still be sounding after deleting track 0, was {peak_after}"
        );

        app.handle(Event::AddTrack {
            kind: crate::models::TrackKind::Midi,
        });
        app.handle(Event::DeleteTrack { track: 1 });
        std::thread::sleep(std::time::Duration::from_millis(50));
        let peak_after_later = app.engine.shared().peaks()[0];
        assert!(
            peak_after_later > 0.01,
            "track 0 should still be sounding after deleting later track 1, was {peak_after_later}"
        );
    }

    #[test]
    fn reordering_tracks_moves_track_and_preserves_color() {
        let mut app = headless_app();
        assert_eq!(app.song.tracks[0].color_index, Some(0));
        assert_eq!(app.song.tracks[1].color_index, Some(1));

        app.handle(Event::SetTrackColor {
            track: 0,
            color_index: 7,
        });
        assert_eq!(app.song.tracks[0].color_index, Some(7));
        assert_eq!(app.ui_state().tracks[0].color_index, 7);

        app.handle(Event::ReorderTrack { from: 0, to: 2 });
        assert_eq!(app.song.tracks[2].color_index, Some(7));
        assert_eq!(app.ui_state().tracks[2].color_index, 7);
        assert_eq!(app.song.tracks[0].color_index, Some(1));
        assert_eq!(app.ui_state().tracks[0].color_index, 1);

        app.handle(Event::Undo);
        assert_eq!(app.song.tracks[0].color_index, Some(7));
        assert_eq!(app.ui_state().tracks[0].color_index, 7);
    }
}
