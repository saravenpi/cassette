//! The song: tracks and tempo.

use serde::{Deserialize, Serialize};

use crate::core::{MAX_BLOCKS, MAX_TRACKS};
use crate::models::track::TrackKind;
use crate::models::{Instrument, Track};
use crate::utils::math::clamp;
use crate::utils::time::Tempo;

/// Project file format version this build writes and understands.
pub const PROJECT_VERSION: u32 = 1;

fn default_version() -> u32 {
    PROJECT_VERSION
}

/// A whole session.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    #[serde(default = "default_version")]
    version: u32,
    bpm: f32,
    pub tracks: Vec<Track>,
}

impl Default for Song {
    fn default() -> Self {
        Self::new()
    }
}

impl Song {
    /// A new session: two MIDI tracks and two audio tracks, all empty.
    pub fn new() -> Self {
        let midi = |name: &str| Track::new(name, TrackKind::Midi, Instrument::preset("Lead"));
        let audio = |name: &str| Track::new(name, TrackKind::Audio, Instrument::preset("Pluck"));
        Self {
            version: PROJECT_VERSION,
            bpm: Tempo::default().bpm,
            tracks: vec![
                midi("Midi 1"),
                midi("Midi 2"),
                audio("Audio 1"),
                audio("Audio 2"),
            ],
        }
    }

    /// Current tempo in beats per minute.
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Set the tempo, clamped to a usable range.
    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = clamp(bpm, 20.0, 400.0);
    }

    /// Validate a song parsed from an untrusted file and bring it inside the
    /// app's limits.
    ///
    /// Returns an error when the file cannot be made safe (a newer format, no
    /// tracks, or a non-finite number). Otherwise returns one message per
    /// correction so the caller can report them.
    pub fn sanitize(&mut self) -> Result<Vec<String>, String> {
        if self.version > PROJECT_VERSION {
            return Err(format!(
                "project format v{} is newer than v{PROJECT_VERSION}",
                self.version
            ));
        }
        if !self.bpm.is_finite() {
            return Err("project tempo is not a finite number".into());
        }
        if self.tracks.is_empty() {
            return Err("project has no tracks".into());
        }
        let mut problems = Vec::new();
        let bpm = Tempo::new(self.bpm).bpm;
        if bpm != self.bpm {
            problems.push(format!("tempo {} clamped to {bpm}", self.bpm));
            self.bpm = bpm;
        }
        if self.tracks.len() > MAX_TRACKS {
            problems.push(format!(
                "dropped {} track(s) past the limit of {MAX_TRACKS}",
                self.tracks.len() - MAX_TRACKS
            ));
            self.tracks.truncate(MAX_TRACKS);
        }
        for track in &mut self.tracks {
            problems.extend(track.sanitize(MAX_BLOCKS)?);
        }
        self.version = PROJECT_VERSION;
        Ok(problems)
    }

    /// Whether any track is soloed.
    pub fn any_solo(&self) -> bool {
        self.tracks.iter().any(|t| t.solo)
    }

    /// How many blocks the whole session holds.
    pub fn block_count(&self) -> usize {
        self.tracks.iter().map(|t| t.blocks.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_song_has_two_midi_and_two_audio_tracks() {
        let song = Song::new();
        assert_eq!(song.tracks.len(), 4);
        let midi = song
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Midi)
            .count();
        assert_eq!(midi, 2);
        assert!(song.tracks.iter().all(|t| t.blocks.is_empty()));
        assert_eq!(song.block_count(), 0);
    }

    #[test]
    fn bpm_is_clamped() {
        let mut song = Song::new();
        song.set_bpm(9_999.0);
        assert_eq!(song.bpm(), 400.0);
        song.set_bpm(1.0);
        assert_eq!(song.bpm(), 20.0);
    }

    #[test]
    fn files_without_a_version_still_load() {
        let json = serde_json::to_string(&Song::new()).expect("serialize");
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("parse");
        value.as_object_mut().expect("object").remove("version");
        let mut song: Song = serde_json::from_value(value).expect("legacy file");
        assert!(song.sanitize().expect("valid").is_empty());
    }

    #[test]
    fn sanitize_rejects_hostile_songs() {
        let mut song = Song::new();
        song.version = PROJECT_VERSION + 1;
        assert!(song.sanitize().is_err());
        let mut song = Song::new();
        song.bpm = f32::NAN;
        assert!(song.sanitize().is_err());
        let mut song = Song::new();
        song.tracks.clear();
        assert!(song.sanitize().is_err());
    }

    #[test]
    fn sanitize_clamps_tempo_and_caps_tracks() {
        let mut song = Song::new();
        song.bpm = -5.0;
        for _ in 0..40 {
            song.tracks
                .push(Track::new("x", TrackKind::Midi, Instrument::preset("Lead")));
        }
        let problems = song.sanitize().expect("fixable");
        assert_eq!(song.bpm(), 20.0);
        assert_eq!(song.tracks.len(), MAX_TRACKS);
        assert_eq!(problems.len(), 2);
    }

    #[test]
    fn solo_is_reported_across_tracks() {
        let mut song = Song::new();
        assert!(!song.any_solo());
        song.tracks[1].solo = true;
        assert!(song.any_solo());
    }
}
