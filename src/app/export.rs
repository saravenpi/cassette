//! Project audio export and file recording.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::App;
use crate::core::render::{RenderCore, RtCommand};
use crate::core::sample::SampleMsg;
use crate::core::wav;

impl App {
    pub(crate) fn export_audio(&mut self, target_path: Option<String>) {
        let path = target_path
            .map(PathBuf::from)
            .unwrap_or_else(|| default_export_path("export", "wav"));
        let sample_rate = 44_100;
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
        let seconds = total_beats * (60.0 / self.song.bpm());
        let total_frames = (seconds * sample_rate as f32) as usize;

        let mut core = RenderCore::new(&self.song, sample_rate);
        for (&slot, frames) in &self.loaded_samples {
            core.push_sample(SampleMsg::Load {
                slot,
                frames: Arc::clone(frames),
                rate: sample_rate as f32,
            });
        }
        for track in 0..self.song.tracks.len() {
            core.set_blocks(track, self.mirror(track));
        }

        core.apply(RtCommand::Seek { beats: 0.0 });
        core.apply(RtCommand::Play);

        let mut output = Vec::with_capacity(total_frames * 2);
        let mut buffer = [0.0f32; 1024];
        let mut frames_rendered = 0;

        while frames_rendered < total_frames {
            let chunk_frames = (total_frames - frames_rendered).min(buffer.len() / 2);
            let slice = &mut buffer[..chunk_frames * 2];
            slice.fill(0.0);
            core.process(slice);
            output.extend_from_slice(slice);
            frames_rendered += chunk_frames;
        }

        match wav::encode_pcm16(&path, &output, 2, sample_rate) {
            Ok(()) => self.status = format!("exported audio to {}", path.display()),
            Err(error) => self.status = format!("export audio failed: {error}"),
        }
    }

    pub(crate) fn record_audio(&mut self, target_path: Option<String>) {
        let path = target_path
            .map(PathBuf::from)
            .unwrap_or_else(|| default_export_path("recording", "wav"));
        self.export_audio(Some(path.to_string_lossy().into_owned()));
        self.status = format!("recorded project to {}", path.display());
    }

    pub(crate) fn export_project(&mut self, target_path: Option<String>) {
        let path = target_path
            .map(PathBuf::from)
            .unwrap_or_else(|| default_export_path("project", "json"));
        match serde_json::to_string_pretty(&self.song) {
            Ok(json) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, json) {
                    Ok(()) => self.status = format!("exported project to {}", path.display()),
                    Err(error) => self.status = format!("export project failed: {error}"),
                }
            }
            Err(error) => self.status = format!("serializing project failed: {error}"),
        }
    }
}

fn default_export_path(prefix: &str, ext: &str) -> PathBuf {
    let base = if let Ok(dir) = std::env::var("ZBOUB_SAMPLES_DIR")
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
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    base.join(format!("zboub69-{prefix}-{timestamp}.{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headless_app() -> App {
        App::new_silent().expect("silent app should build")
    }

    #[test]
    fn audio_export_writes_valid_wav() {
        let mut app = headless_app();
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("zboub69_test_export.wav");
        app.export_audio(Some(path.to_string_lossy().into_owned()));
        assert!(path.exists());
        let meta = std::fs::metadata(&path).expect("metadata should exist");
        assert!(meta.len() > 44);
        let _ = std::fs::remove_file(&path);
        assert!(app.status.contains("exported audio to"));
    }

    #[test]
    fn project_export_writes_valid_json() {
        let mut app = headless_app();
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("zboub69_test_export.json");
        app.export_project(Some(path.to_string_lossy().into_owned()));
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).expect("read content");
        assert!(content.contains("tracks"));
        assert!(content.contains("bpm"));
        let _ = std::fs::remove_file(&path);
        assert!(app.status.contains("exported project to"));
    }
}
