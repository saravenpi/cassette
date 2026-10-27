//! Project audio export and file recording.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use super::App;
use crate::core::meter::MeterBank;
use crate::core::render::{RenderCore, RtCommand};
use crate::core::sample::SampleMsg;
use crate::core::wav;
use crate::models::{EffectChain, Song};
use crate::ui::view::Cursor;

const EXPORT_SAMPLE_RATE: u32 = 44_100;
const MAX_EXPORT_SECONDS: f32 = 600.0;

impl App {
    fn report_export_progress(&mut self, progress: Option<f32>) {
        self.export_progress = progress;
        if let Some(reporter) = &self.progress_reporter {
            reporter(progress);
        }
    }

    pub(crate) fn export_audio(&mut self, target_path: Option<String>) {
        let path = target_path
            .map(PathBuf::from)
            .unwrap_or_else(|| default_export_path("export", "wav"));
        self.status = match self.render_to(&path) {
            Ok(truncated) => format!(
                "exported audio to {}{}",
                path.display(),
                if truncated {
                    " (cut at 10 minutes)"
                } else {
                    ""
                }
            ),
            Err(error) => format!("export audio failed: {error:#}"),
        };
        self.report_export_progress(None);
    }

    fn render_to(&mut self, path: &Path) -> Result<bool> {
        let sample_rate = EXPORT_SAMPLE_RATE;
        let max_end_beats = self
            .song
            .tracks
            .iter()
            .flat_map(|t| t.blocks.iter())
            .map(|b| b.end_beats())
            .fold(0.0f32, f32::max);
        let active_beats = if max_end_beats > 0.0 {
            max_end_beats
        } else {
            16.0
        };
        let total_beats = active_beats + 2.0;
        let wanted = total_beats * (60.0 / self.song.bpm());
        let seconds = wanted.min(MAX_EXPORT_SECONDS);
        let total_frames = (seconds * sample_rate as f32) as usize;

        let mut core = RenderCore::new(&self.song, sample_rate);
        let rates = self.source_rates();
        for (&slot, frames) in &self.loaded_samples {
            let rate = rates.get(&slot).copied().unwrap_or(sample_rate as f32);
            core.push_sample(SampleMsg::Load {
                slot,
                frames: Arc::clone(frames),
                rate,
            });
            core.apply(RtCommand::SetSampleRate { slot, rate });
        }
        for track in 0..self.song.tracks.len() {
            core.set_blocks(track, self.mirror(track));
            core.set_notes(track, self.mirror_notes(track));
            core.apply(RtCommand::SetEffects {
                track: track as u8,
                chain: EffectChain::from_effects(&self.song.tracks[track].effects),
            });
        }

        core.apply(RtCommand::SetLooping { looping: false });
        core.apply(RtCommand::Seek { beats: 0.0 });
        core.apply(RtCommand::Play);

        self.report_export_progress(Some(0.0));

        let mut output = Vec::with_capacity(total_frames * 2);
        let mut buffer = [0.0f32; 1024];
        let mut frames_rendered = 0;
        let mut last_pct = 0;

        while frames_rendered < total_frames {
            let chunk_frames = (total_frames - frames_rendered).min(buffer.len() / 2);
            let slice = &mut buffer[..chunk_frames * 2];
            slice.fill(0.0);
            core.process(slice);
            output.extend_from_slice(slice);
            frames_rendered += chunk_frames;

            if total_frames > 0 {
                let pct = (frames_rendered * 100 / total_frames) as u32;
                if pct != last_pct {
                    last_pct = pct;
                    self.report_export_progress(Some(pct as f32 / 100.0));
                }
            }
        }

        write_atomic(path, |temp| {
            wav::encode_pcm16(temp, &output, 2, sample_rate)
        })?;
        Ok(wanted > MAX_EXPORT_SECONDS)
    }

    fn source_rates(&self) -> HashMap<u8, f32> {
        self.samples
            .iter()
            .filter_map(|(source, &slot)| {
                let sample = wav::decode(Path::new(source), usize::MAX).ok()?;
                Some((slot, sample.sample_rate as f32))
            })
            .collect()
    }

    pub(crate) fn export_project(&mut self, target_path: Option<String>) {
        let path = if let Some(target) = target_path {
            let mut p = PathBuf::from(target);
            if p.extension().is_none() {
                p.set_extension("cassette");
            }
            p
        } else if let Some(remembered) = self.project_path.clone() {
            remembered
        } else {
            default_export_path("project", "cassette")
        };
        let json = match serde_json::to_string_pretty(&self.song) {
            Ok(json) => json,
            Err(error) => {
                self.status = format!("serializing project failed: {error}");
                return;
            }
        };
        self.status = match write_atomic(&path, |temp| {
            std::fs::write(temp, json).with_context(|| format!("writing {}", temp.display()))
        }) {
            Ok(()) => {
                self.project_path = Some(path.clone());
                self.saved_revision = self.edit_revision;
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("project.cassette");
                format!("saved {file_name}")
            }
            Err(error) => format!("export project failed: {error:#}"),
        };
    }

    pub(crate) fn load_project(&mut self, target_path: Option<String>) {
        let path = if let Some(target) = target_path {
            let mut p = PathBuf::from(target);
            if p.extension().is_none() {
                if !p.exists()
                    && !p.with_extension("cassette").exists()
                    && p.with_extension("json").exists()
                {
                    p.set_extension("json");
                } else {
                    p.set_extension("cassette");
                }
            }
            Some(p)
        } else {
            latest_project_path()
        };
        let Some(path) = path else {
            self.status = "no saved project found to load".into();
            return;
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                self.status = format!("load project failed: {error}");
                return;
            }
        };
        let mut song: Song = match serde_json::from_str(&content) {
            Ok(song) => song,
            Err(error) => {
                self.status = format!("parsing project failed: {error}");
                return;
            }
        };
        let problems = match song.sanitize() {
            Ok(problems) => problems,
            Err(error) => {
                self.status = format!("project rejected: {error}");
                return;
            }
        };

        self.reset_session();
        self.song = song;
        self.meters = MeterBank::new(self.song.tracks.len());

        let mut sources: Vec<String> = self
            .song
            .tracks
            .iter()
            .flat_map(|t| t.blocks.iter())
            .filter(|b| !b.source.is_empty())
            .map(|b| b.source.clone())
            .collect();
        sources.sort_unstable();
        sources.dedup();
        let missing: Vec<String> = sources
            .into_iter()
            .filter(|source| self.load_sample_source(source).is_none())
            .collect();

        self.resync();

        self.project_path = Some(path.clone());
        self.saved_revision = self.edit_revision;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project.cassette");
        let mut status = format!("loaded {file_name}");
        if !problems.is_empty() {
            status.push_str(&format!("; fixed: {}", problems.join(", ")));
        }
        if !missing.is_empty() {
            status.push_str(&format!(
                "; {} sample(s) could not be loaded: {}",
                missing.len(),
                missing.join(", ")
            ));
        }
        self.status = status;
    }

    fn reset_session(&mut self) {
        self.held.clear();
        self.rename = None;
        self.cursor = Cursor::default();
        self.send(RtCommand::AllNotesOff);
        self.send(RtCommand::Stop);
        if let Some(producer) = self.producer.as_mut() {
            for &slot in self.samples.values() {
                let _ = producer.push(SampleMsg::Free { slot });
            }
        }
        self.samples.clear();
        self.loaded_samples.clear();
    }
}

fn write_atomic(path: &Path, write: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("creating directory {}", parent.display()))?;
    let name = path
        .file_name()
        .map_or_else(|| "out".into(), |n| n.to_string_lossy().into_owned());
    let temp = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = write(&temp).and_then(|()| {
        std::fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn default_directory() -> PathBuf {
    let env_dir = std::env::var("CASSETTE_SAMPLES_DIR")
        .or_else(|_| std::env::var("ZBOUB_SAMPLES_DIR"))
        .ok();
    if let Some(dir) = env_dir
        && !dir.is_empty()
    {
        PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        let music = Path::new(&home).join("Music");
        if music.is_dir() {
            music
        } else {
            PathBuf::from(home)
        }
    } else {
        PathBuf::from(".")
    }
}

fn default_export_path(prefix: &str, ext: &str) -> PathBuf {
    let base = default_directory();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    base.join(format!("cassette-{prefix}-{timestamp}.{ext}"))
}

fn latest_project_path() -> Option<PathBuf> {
    let base = default_directory();
    let entries = std::fs::read_dir(&base).ok()?;
    let mut projects: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_name()?.to_str()?;
            if (name.starts_with("cassette-project-") || name.starts_with("zboub69-project-"))
                && (name.ends_with(".cassette") || name.ends_with(".json"))
            {
                let mtime = e.metadata().ok()?.modified().ok()?;
                Some((mtime, path))
            } else {
                None
            }
        })
        .collect();
    projects.sort_by_key(|b| std::cmp::Reverse(b.0));
    projects.into_iter().next().map(|(_, path)| path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "cassette-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn file(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        fn arg(&self, name: &str) -> Option<String> {
            Some(self.file(name).to_string_lossy().into_owned())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn audio_export_writes_valid_wav() {
        let mut app = headless_app();
        let dir = TempDir::new();
        app.export_audio(dir.arg("out.wav"));
        let meta = std::fs::metadata(dir.file("out.wav")).expect("metadata should exist");
        assert!(meta.len() > 44);
        assert!(app.status.contains("exported audio to"));
        let leftovers = std::fs::read_dir(&dir.0).expect("list").count();
        assert_eq!(leftovers, 1, "the temp file must be renamed away");
    }

    #[test]
    fn a_failed_render_reports_failure_not_success() {
        let mut app = headless_app();
        let dir = TempDir::new();
        std::fs::write(dir.file("blocker"), b"x").expect("write blocker");
        app.export_audio(Some(
            dir.file("blocker/out.wav").to_string_lossy().into_owned(),
        ));
        assert!(app.status.contains("failed"), "{}", app.status);
    }

    #[test]
    fn project_export_writes_valid_json() {
        let mut app = headless_app();
        let dir = TempDir::new();
        app.export_project(dir.arg("nested/p.json"));
        let content = std::fs::read_to_string(dir.file("nested/p.json")).expect("read content");
        assert!(content.contains("tracks"));
        assert!(content.contains("bpm"));
        assert!(content.contains("\"version\""));
        assert!(app.status.contains("saved p.json"));
    }

    #[test]
    fn project_load_restores_song_and_tempo() {
        let mut app = headless_app();
        app.song.set_bpm(142.0);
        let dir = TempDir::new();
        app.export_project(dir.arg("p.json"));

        let mut fresh = headless_app();
        assert_eq!(fresh.song.bpm(), 120.0);
        fresh.load_project(dir.arg("p.json"));
        assert_eq!(fresh.song.bpm(), 142.0);
        assert!(fresh.status.contains("loaded p.json"));
    }

    #[test]
    fn a_loop_region_is_set_cleared_and_saved_with_the_project() {
        let mut app = headless_app();
        app.submit(crate::events::Event::SetLoopRegion {
            start_beats: 8.0,
            end_beats: 4.0,
        });
        let region = app.ui_state().loop_region.expect("a region is set");
        assert_eq!((region.start_beats, region.end_beats), (4.0, 8.0));

        let dir = TempDir::new();
        app.export_project(dir.arg("p.json"));
        let mut fresh = headless_app();
        fresh.load_project(dir.arg("p.json"));
        assert_eq!(fresh.song.loop_region(), Some(region));

        fresh.submit(crate::events::Event::SetLoopRegion {
            start_beats: 2.0,
            end_beats: 2.0,
        });
        assert_eq!(fresh.ui_state().loop_region, None, "a click clears it");
    }

    fn load_json(json: &str) -> App {
        let dir = TempDir::new();
        std::fs::write(dir.file("p.json"), json).expect("write project");
        let mut app = headless_app();
        app.load_project(dir.arg("p.json"));
        app
    }

    #[test]
    fn empty_and_garbage_files_are_refused_and_change_nothing() {
        for json in ["", "{}", "[]", "not json", "{\"bpm\":120,\"tracks\":[]}"] {
            let app = load_json(json);
            assert_eq!(app.song.tracks.len(), 4, "{json:?}");
            assert!(!app.status.contains("loaded"), "{json:?}: {}", app.status);
        }
    }

    #[test]
    fn hostile_numbers_are_refused_or_clamped() {
        let app = load_json("{\"bpm\":1e999,\"tracks\":[]}");
        assert!(!app.status.contains("loaded"));

        let mut song = Song::new();
        song.set_bpm(120.0);
        let mut value = serde_json::to_value(&song).expect("to value");
        value["bpm"] = serde_json::json!(-4.0);
        value["version"] = serde_json::json!(1);
        value["tracks"][0]["volume"] = serde_json::json!(99.0);
        value["tracks"][0]["pan"] = serde_json::json!(-99.0);
        let block = serde_json::json!({
            "name": "b", "kind": "midi", "startBeats": 1e30, "lengthBeats": -5.0, "source": ""
        });
        let late = serde_json::json!({
            "name": "a", "kind": "midi", "startBeats": 8.0, "lengthBeats": 1e30, "source": ""
        });
        value["tracks"][0]["blocks"] = serde_json::json!([block, late]);
        let app = load_json(&value.to_string());
        assert!(app.status.contains("loaded"), "{}", app.status);
        assert!(app.status.contains("fixed"), "{}", app.status);
        assert_eq!(app.song.bpm(), 20.0);
        let track = &app.song.tracks[0];
        assert_eq!(track.volume, 1.0);
        assert_eq!(track.pan, -1.0);
        assert_eq!(track.blocks[0].start_beats, 8.0);
        assert_eq!(
            track.blocks[0].length_beats,
            crate::models::MAX_LENGTH_BEATS
        );
        assert_eq!(track.blocks[1].start_beats, crate::models::MAX_START_BEATS);
    }

    #[test]
    fn too_many_blocks_and_future_versions_are_handled() {
        let mut value = serde_json::to_value(Song::new()).expect("to value");
        let blocks: Vec<_> = (0..200)
            .map(|i| {
                serde_json::json!({
                    "name": "b", "kind": "midi",
                    "startBeats": 200 - i, "lengthBeats": 1.0, "source": ""
                })
            })
            .collect();
        value["tracks"][1]["blocks"] = serde_json::Value::Array(blocks);
        let app = load_json(&value.to_string());
        let lane = &app.song.tracks[1].blocks;
        assert_eq!(lane.len(), crate::core::MAX_BLOCKS);
        assert!(
            lane.windows(2)
                .all(|w| w[0].start_beats <= w[1].start_beats)
        );

        value["version"] = serde_json::json!(99);
        let app = load_json(&value.to_string());
        assert!(app.status.contains("rejected"), "{}", app.status);
    }

    #[test]
    fn loading_resets_session_state_and_reports_missing_samples() {
        let mut value = serde_json::to_value(Song::new()).expect("to value");
        value["tracks"][2]["blocks"] = serde_json::json!([{
            "name": "gone", "kind": "audio", "startBeats": 0.0,
            "lengthBeats": 4.0, "source": "/nonexistent/gone.wav"
        }]);
        let dir = TempDir::new();
        std::fs::write(dir.file("p.json"), value.to_string()).expect("write");
        let mut app = headless_app();
        app.handle(crate::events::Event::NoteOn {
            pitch: 60,
            velocity: 100,
        });
        app.rename = Some("half typed".into());
        app.cursor.track = 3;
        app.samples.insert("old.wav".into(), 5);
        app.loaded_samples.insert(5, Arc::from(vec![0.0f32; 4]));
        app.load_project(dir.arg("p.json"));
        assert!(app.held.is_empty());
        assert!(app.rename.is_none());
        assert_eq!(app.cursor, Cursor::default());
        assert!(app.samples.is_empty());
        assert!(app.loaded_samples.is_empty());
        assert!(app.status.contains("could not be loaded"), "{}", app.status);
        assert!(app.status.contains("gone.wav"));
    }

    fn add_note(app: &mut App, start_beats: f32) {
        app.submit(crate::events::Event::AddNote {
            track: 0,
            block: 0,
            pitch: 64,
            start_beats,
            length_beats: 1.0,
            velocity: 90,
        });
    }

    fn app_with_notes() -> App {
        let mut app = headless_app();
        app.submit(crate::events::Event::CreateBlock {
            track: 0,
            start_beats: 4.0,
            length_beats: 4.0,
        });
        add_note(&mut app, 2.0);
        add_note(&mut app, 0.5);
        app
    }

    #[test]
    fn notes_survive_a_save_and_load_in_drawing_order() {
        let app = app_with_notes();
        let saved = app.song.tracks[0].blocks[0].notes.clone();
        assert_eq!(saved.len(), 2);
        let mut source = app;
        let dir = TempDir::new();
        source.export_project(dir.arg("p.json"));
        let content = std::fs::read_to_string(dir.file("p.json")).expect("read content");
        assert!(content.contains("\"version\": 1"), "{content}");

        let mut fresh = headless_app();
        fresh.load_project(dir.arg("p.json"));
        assert!(fresh.status.contains("loaded"), "{}", fresh.status);
        assert!(!fresh.status.contains("fixed"), "{}", fresh.status);
        assert_eq!(fresh.song.tracks[0].blocks[0].notes, saved);
        assert_eq!(fresh.mirror_notes(0)[0].start_beats, 4.5);
    }

    #[test]
    fn an_effect_chain_survives_a_save_and_load() {
        use crate::models::EffectKind;
        let mut app = headless_app();
        for kind in [EffectKind::Delay, EffectKind::Filter] {
            app.submit(crate::events::Event::AddEffect {
                track: 0,
                kind,
                index: None,
            });
        }
        app.submit(crate::events::Event::SetEffectParam {
            track: 0,
            index: 1,
            param: 0,
            value: 2.0,
        });
        app.submit(crate::events::Event::SetEffectEnabled {
            track: 0,
            index: 0,
            enabled: false,
        });
        let dir = TempDir::new();
        app.export_project(dir.arg("p.json"));
        app.export_audio(dir.arg("out.wav"));
        assert!(app.status.contains("exported audio to"), "{}", app.status);

        let mut fresh = headless_app();
        fresh.load_project(dir.arg("p.json"));
        assert!(!fresh.status.contains("fixed"), "{}", fresh.status);
        assert_eq!(fresh.song.tracks[0].effects, app.song.tracks[0].effects);

        let mut value = serde_json::to_value(&app.song).expect("to value");
        value["tracks"][0]
            .as_object_mut()
            .expect("track")
            .remove("effects");
        let old = load_json(&value.to_string());
        assert!(old.status.contains("loaded"), "{}", old.status);
        assert!(old.song.tracks[0].effects.is_empty());
    }

    #[test]
    fn a_v0_7_0_project_without_notes_loads() {
        let mut value = serde_json::to_value(app_with_notes().song).expect("to value");
        for track in value["tracks"].as_array_mut().expect("tracks") {
            for block in track["blocks"].as_array_mut().expect("blocks") {
                block.as_object_mut().expect("block").remove("notes");
            }
        }
        assert!(!value.to_string().contains("notes"));
        let app = load_json(&value.to_string());
        assert!(app.status.contains("loaded"), "{}", app.status);
        assert_eq!(app.song.tracks[0].blocks.len(), 1);
        assert!(app.song.tracks[0].blocks[0].notes.is_empty());
    }

    #[test]
    fn an_exported_note_sounds_at_its_time_and_not_before() {
        let mut app = headless_app();
        app.submit(crate::events::Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 8.0,
        });
        add_note(&mut app, 4.0);
        let dir = TempDir::new();
        app.export_audio(dir.arg("out.wav"));
        assert!(app.status.contains("exported audio to"), "{}", app.status);

        let sample = wav::decode(&dir.file("out.wav"), usize::MAX).expect("decode export");
        let frames_per_beat = EXPORT_SAMPLE_RATE as f32 * 60.0 / app.song.bpm();
        let at = |beats: f32| (beats * frames_per_beat) as usize;
        let loudest = |from: f32, to: f32| {
            sample.frames[at(from)..at(to)]
                .iter()
                .fold(0.0f32, |peak, v| peak.max(v.abs()))
        };
        assert!(loudest(0.0, 3.9) < 0.001, "silent before the note");
        assert!(loudest(4.0, 5.0) > 0.01, "audible during the note");
    }

    #[test]
    fn an_added_delay_echoes_in_the_export() {
        let mut app = headless_app();
        app.submit(crate::events::Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 1.0,
        });
        add_note(&mut app, 0.0);
        app.submit(crate::events::Event::AddEffect {
            track: 0,
            kind: crate::models::EffectKind::Delay,
            index: None,
        });
        let dir = TempDir::new();
        app.export_audio(dir.arg("out.wav"));
        let sample = wav::decode(&dir.file("out.wav"), usize::MAX).expect("decode export");
        let frames_per_beat = EXPORT_SAMPLE_RATE as f32 * 60.0 / app.song.bpm();
        let tail = &sample.frames[(2.0 * frames_per_beat) as usize..];
        let peak = tail.iter().fold(0.0f32, |peak, v| peak.max(v.abs()));
        assert!(
            peak > 0.001,
            "the delay keeps sounding after the note: {peak}"
        );
    }

    #[test]
    fn an_added_sidechain_compressor_ducks_the_export() {
        let mut app = headless_app();
        app.submit(crate::events::Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 1.0,
        });
        add_note(&mut app, 0.0);
        app.submit(crate::events::Event::SetTrackMix {
            track: 0,
            volume: 1.0,
            pan: 0.0,
            muted: true,
            solo: false,
        });

        app.submit(crate::events::Event::CreateBlock {
            track: 1,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        app.submit(crate::events::Event::AddNote {
            track: 1,
            block: 0,
            pitch: 60,
            start_beats: 0.0,
            length_beats: 4.0,
            velocity: 100,
        });
        app.submit(crate::events::Event::AddEffect {
            track: 1,
            kind: crate::models::EffectKind::Compressor,
            index: None,
        });
        app.submit(crate::events::Event::SetEffectSidechain {
            track: 1,
            index: 0,
            source: Some(0),
        });
        app.submit(crate::events::Event::SetEffectParam {
            track: 1,
            index: 0,
            param: 0,
            value: -20.0,
        });
        app.submit(crate::events::Event::SetEffectParam {
            track: 1,
            index: 0,
            param: 1,
            value: 10.0,
        });
        app.submit(crate::events::Event::SetEffectParam {
            track: 1,
            index: 0,
            param: 2,
            value: 0.1,
        });
        app.submit(crate::events::Event::SetEffectParam {
            track: 1,
            index: 0,
            param: 3,
            value: 50.0,
        });

        let dir = TempDir::new();
        app.export_audio(dir.arg("out.wav"));
        let sample = wav::decode(&dir.file("out.wav"), usize::MAX).expect("decode export");
        let frames_per_beat = (EXPORT_SAMPLE_RATE as f32 * 60.0 / app.song.bpm()) as usize;
        let ducked = &sample.frames[frames_per_beat / 4..frames_per_beat];
        let recovered = &sample.frames[2 * frames_per_beat..3 * frames_per_beat];
        let peak_ducked = ducked.iter().fold(0.0f32, |peak, v| peak.max(v.abs()));
        let peak_recovered = recovered.iter().fold(0.0f32, |peak, v| peak.max(v.abs()));
        assert!(peak_recovered > 0.05);
        assert!(peak_ducked < peak_recovered * 0.7);
    }

    fn audible_span(wav_path: &Path) -> (usize, usize, usize) {
        let sample = wav::decode(wav_path, usize::MAX).expect("decode export");
        let frames = &sample.frames;
        let first = frames.iter().position(|v| v.abs() > 0.01).unwrap_or(0);
        let last = frames.iter().rposition(|v| v.abs() > 0.01).unwrap_or(0);
        let crossings = frames[first..=last]
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        (first, last, crossings)
    }

    #[test]
    fn a_22k_sample_exports_at_its_own_speed_and_pitch() {
        let dir = TempDir::new();
        let source_rate = 22_050u32;
        let tone: Vec<f32> = (0..source_rate)
            .map(|i| (i as f32 / source_rate as f32 * 441.0 * std::f32::consts::TAU).sin() * 0.8)
            .collect();
        let stereo: Vec<f32> = tone.iter().flat_map(|&v| [v, v]).collect();
        wav::encode_pcm16(&dir.file("tone.wav"), &stereo, 2, source_rate).expect("write tone");

        let mut app = headless_app();
        app.submit(crate::events::Event::ImportSample {
            track: 2,
            path: dir.file("tone.wav").to_string_lossy().into_owned(),
            start_beats: 0.0,
        });
        assert_eq!(app.song.block_count(), 1, "{}", app.status);
        app.export_audio(dir.arg("out.wav"));
        assert!(app.status.contains("exported audio to"), "{}", app.status);

        let (first, last, crossings) = audible_span(&dir.file("out.wav"));
        let seconds = (last - first) as f32 / EXPORT_SAMPLE_RATE as f32;
        assert!((seconds - 1.0).abs() < 0.05, "played for {seconds} s");
        let hertz = crossings as f32 / 2.0 / seconds;
        assert!((hertz - 441.0).abs() < 15.0, "pitch was {hertz} Hz");
    }

    #[test]
    fn project_path_dirty_flag_and_cassette_extension() {
        let mut app = headless_app();
        let state = app.ui_state();
        assert_eq!(state.project_path, None);
        assert!(!state.project_dirty, "fresh app is clean");

        app.submit(crate::events::Event::CreateBlock {
            track: 0,
            start_beats: 0.0,
            length_beats: 4.0,
        });
        assert!(app.ui_state().project_dirty, "dirty after edit");

        let dir = TempDir::new();
        let base_name = dir.arg("my_session");
        app.export_project(base_name.clone());
        assert_eq!(app.status, "saved my_session.cassette");
        let expected_path = dir.file("my_session.cassette");
        assert!(expected_path.exists());
        assert_eq!(app.project_path, Some(expected_path.clone()));
        assert_eq!(
            app.ui_state().project_path,
            Some(expected_path.display().to_string())
        );
        assert!(!app.ui_state().project_dirty, "clean after save");

        app.submit(crate::events::Event::Undo);
        assert!(app.ui_state().project_dirty, "undo after save is dirty");

        app.export_project(None);
        assert_eq!(app.status, "saved my_session.cassette");
        assert!(
            !app.ui_state().project_dirty,
            "clean after saving again to remembered path"
        );

        let mut fresh = headless_app();
        fresh.load_project(base_name);
        assert_eq!(fresh.status, "loaded my_session.cassette");
        assert_eq!(fresh.project_path, Some(expected_path));
        assert!(!fresh.ui_state().project_dirty, "clean after loading");
    }
}
