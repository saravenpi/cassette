//! The song: tracks and tempo.

use serde::{Deserialize, Serialize};

use crate::models::track::TrackKind;
use crate::models::{Instrument, Track};
use crate::utils::math::clamp;
use crate::utils::time::Tempo;

/// A whole session.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
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
    fn solo_is_reported_across_tracks() {
        let mut song = Song::new();
        assert!(!song.any_solo());
        song.tracks[1].solo = true;
        assert!(song.any_solo());
    }
}
