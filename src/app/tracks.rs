//! Track state: selection, kind, mixer strip and the track's voice.
//!
//! Every edit here lands on the song first and reaches the audio thread second,
//! through the same `RtCommand` a resync would use.

use super::App;
use crate::core::render::RtCommand;
use crate::models::Instrument;

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
    pub(crate) fn add_track(&mut self, _kind: crate::models::TrackKind) {}

    pub(crate) fn delete_track(&mut self, _track: usize) {}

    pub(crate) fn duplicate_track(&mut self, _track: usize) {}

    pub(crate) fn rename_track(&mut self, _track: usize, _name: &str) {}

    pub(crate) fn set_instrument_enabled(&mut self, _track: usize, _enabled: bool) {}

    pub(crate) fn set_instrument_tone(
        &mut self,
        _track: usize,
        _cutoff: f32,
        _resonance: f32,
        _detune: f32,
        _sub: f32,
    ) {
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
}
