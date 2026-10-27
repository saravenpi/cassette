//! The song: tracks and tempo.

use serde::{Deserialize, Serialize};

use crate::core::{MAX_BLOCKS, MAX_TRACKS};
use crate::models::track::TrackKind;
use crate::models::{Block, Instrument, MAX_START_BEATS, Track};
use crate::utils::time::Tempo;

/// Project file format version this build writes and understands.
pub const PROJECT_VERSION: u32 = 1;

fn default_version() -> u32 {
    PROJECT_VERSION
}

fn default_next_effect_id() -> u32 {
    1
}

/// The stretch of the timeline the transport loops over, in beats.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopRegion {
    pub start_beats: f32,
    pub end_beats: f32,
}

impl LoopRegion {
    /// A region between two beats in either order, kept on the timeline. `None`
    /// when the span is not finite or shorter than the shortest block, which is
    /// what a click without a drag produces.
    pub fn between(a: f32, b: f32) -> Option<Self> {
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        let start = a.min(b).clamp(0.0, MAX_START_BEATS);
        let end = a.max(b).clamp(0.0, MAX_START_BEATS);
        (end - start >= Block::MIN_LENGTH_BEATS).then_some(Self {
            start_beats: start,
            end_beats: end,
        })
    }
}

/// A whole session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    #[serde(default = "default_version")]
    version: u32,
    bpm: f32,
    pub tracks: Vec<Track>,
    /// Where the transport loops; the whole song when `None`.
    #[serde(default)]
    loop_region: Option<LoopRegion>,
    #[serde(default = "default_next_effect_id")]
    next_effect_id: u32,
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
            loop_region: None,
            next_effect_id: 1,
        }
    }

    /// Next unique effect id in the song.
    pub fn next_effect_id(&mut self) -> u32 {
        let id = self.next_effect_id.max(1);
        self.next_effect_id = id.saturating_add(1);
        id
    }

    /// Current tempo in beats per minute.
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Set the tempo, clamped to a usable range.
    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(20.0, 400.0);
    }

    /// Where the transport loops, or `None` for the whole song.
    pub fn loop_region(&self) -> Option<LoopRegion> {
        self.loop_region
    }

    /// Set or clear the loop region.
    pub fn set_loop_region(&mut self, region: Option<LoopRegion>) {
        self.loop_region = region;
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
        let mut max_id: u32 = 0;
        let mut seen_ids = std::collections::HashSet::new();
        let mut fix_needed = false;
        for track in &self.tracks {
            for effect in &track.effects {
                if effect.id == 0 || !seen_ids.insert(effect.id) {
                    fix_needed = true;
                }
                max_id = max_id.max(effect.id);
            }
        }
        self.next_effect_id = self.next_effect_id.max(max_id.saturating_add(1)).max(1);
        if fix_needed {
            let mut seen = std::collections::HashSet::new();
            let mut next_id = self.next_effect_id;
            for track in &mut self.tracks {
                for effect in &mut track.effects {
                    if effect.id == 0 || !seen.insert(effect.id) {
                        let id = next_id.max(1);
                        next_id = id.saturating_add(1);
                        effect.id = id;
                        seen.insert(effect.id);
                    }
                }
            }
            self.next_effect_id = next_id;
            problems.push("assigned unique effect ids".into());
        }
        let track_count = self.tracks.len();
        for (i, track) in self.tracks.iter_mut().enumerate() {
            for effect in &mut track.effects {
                if !effect.kind.takes_sidechain() {
                    if effect.sidechain.is_some() {
                        effect.sidechain = None;
                        problems.push(format!(
                            "track {:?}: sidechain removed from {}",
                            track.name,
                            effect.kind.name()
                        ));
                    }
                } else if let Some(source) = effect.sidechain
                    && (source as usize >= track_count || source as usize == i)
                {
                    effect.sidechain = None;
                    problems.push(format!("track {:?}: invalid sidechain removed", track.name));
                }
            }
        }
        if let Some(region) = self.loop_region {
            let kept = LoopRegion::between(region.start_beats, region.end_beats);
            if kept != Some(region) {
                problems.push("loop region clamped or dropped".into());
                self.loop_region = kept;
            }
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
    fn a_loop_region_is_ordered_kept_on_the_timeline_and_never_empty() {
        let region = LoopRegion::between(8.0, 4.0).expect("a real span");
        assert_eq!((region.start_beats, region.end_beats), (4.0, 8.0));
        let clamped = LoopRegion::between(-2.0, 1.0).expect("a real span");
        assert_eq!(clamped.start_beats, 0.0);
        assert_eq!(LoopRegion::between(3.0, 3.1), None, "a click is not a loop");
        assert_eq!(LoopRegion::between(f32::NAN, 4.0), None);
    }

    #[test]
    fn a_hostile_loop_region_is_dropped_on_load() {
        let mut song = Song::new();
        song.loop_region = Some(LoopRegion {
            start_beats: 9.0,
            end_beats: 2.0,
        });
        let problems = song.sanitize().expect("a usable song");
        assert_eq!(
            song.loop_region(),
            Some(LoopRegion {
                start_beats: 2.0,
                end_beats: 9.0
            })
        );
        assert!(problems.iter().any(|p| p.contains("loop region")));
    }

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
