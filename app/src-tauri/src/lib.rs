//! The zboub69 desktop shell: a thin Tauri command surface over the engine.
//!
//! The engine owns a cpal `Stream`, and cpal marks `Stream` `!Send` and `!Sync`
//! on every platform, so the engine cannot live in Tauri's managed state. It gets
//! a thread of its own instead:
//!
//! - the webview sends intent down a channel (`send`),
//! - the engine thread applies it and publishes a snapshot once a frame (`state`).
//!
//! A command wakes that thread at once rather than on the next frame, so a note
//! sounds the moment the key goes down. Timing, synthesis and the meters all stay
//! in the engine; the webview only draws.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use crossbeam::channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use tauri::State;

use zboub69::app::{App, WAVEFORM_BUCKETS, peaks};
use zboub69::events::Event;
use zboub69::ui::view::UiState;

/// One frame at 60 Hz, the fastest the webview can draw.
const FRAME: Duration = Duration::from_micros(16_667);
/// Entries one directory listing returns at most.
const MAX_ENTRIES: usize = 2_000;

/// The latest snapshot, published by the engine thread.
type Snapshot = Arc<Mutex<Option<UiState>>>;
/// Intent from the webview, drained by the engine thread.
type Commands = Sender<Event>;

/// The current snapshot, or `None` in the moment before the engine first publishes.
#[tauri::command]
fn state(snapshot: State<'_, Snapshot>) -> Result<Option<UiState>, String> {
    let snapshot = snapshot
        .lock()
        .map_err(|error| format!("zboub69: {error}"))?;
    Ok(snapshot.clone())
}

/// Apply one intent. Resolves as soon as the engine thread has taken it.
#[tauri::command]
fn send(event: Event, commands: State<'_, Commands>) -> Result<(), String> {
    commands
        .send(event)
        .map_err(|_| "zboub69: the engine thread has stopped".to_string())
}

/// Read the stream port out of `ZBOUB_STREAM_PORT`. Only a nonzero `u16` counts;
/// unset, empty, unparseable and zero all mean "no stream".
fn stream_port(raw: Option<&str>) -> Option<u16> {
    raw.and_then(|raw| raw.parse::<u16>().ok())
        .filter(|port| *port > 0)
}

/// What a sample-browser row points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum SampleKind {
    /// A directory the browser can open.
    Dir,
    /// A WAV file the engine can load.
    Wav,
    /// Another audio file format the engine can load.
    Audio,
    /// Anything else, listed but not loadable.
    Other,
}

/// One row of the sample browser.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SampleEntry {
    name: String,
    path: String,
    kind: SampleKind,
    size: u64,
    modified_ms: u64,
}

/// The user's home directory, or the working directory when `HOME` is unset.
#[tauri::command]
fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| ".".into())
}

/// The directory the sample browser opens on: `$ZBOUB_SAMPLES_DIR` when set,
/// else `~/Music` when it exists, else the home directory.
#[tauri::command]
fn samples_root() -> String {
    if let Ok(dir) = std::env::var("ZBOUB_SAMPLES_DIR")
        && !dir.is_empty()
    {
        return dir;
    }
    let home = home_dir();
    let music = Path::new(&home).join("Music");
    if music.is_dir() {
        return music.to_string_lossy().into_owned();
    }
    home
}

/// The parent of `path`, so the browser can go up. `None` at the root.
#[tauri::command]
fn parent_of(path: String) -> Option<String> {
    Path::new(&path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
}

/// List one directory of the sample root: directories first, then names, at
/// most [`MAX_ENTRIES`] of them. A `dir` outside the root is refused.
#[tauri::command]
fn list_samples(dir: Option<String>) -> Result<Vec<SampleEntry>, String> {
    let root = resolve(&samples_root())?;
    let requested = dir.unwrap_or_else(|| root.to_string_lossy().into_owned());
    let target = resolve(&requested)?;
    if !target.starts_with(&root) {
        return Err(format!(
            "zboub69: {requested} is outside the sample root {}",
            root.display()
        ));
    }
    let listing = std::fs::read_dir(&target)
        .map_err(|error| format!("zboub69: cannot read {}: {error}", target.display()))?;
    let mut entries: Vec<SampleEntry> = listing
        .take(MAX_ENTRIES)
        .filter_map(|entry| entry.ok())
        .map(|entry| describe(&entry))
        .collect();
    entries.sort_by_key(|entry| (entry.kind != SampleKind::Dir, entry.name.to_lowercase()));
    Ok(entries)
}

/// Peaks of an audio file for drawing an audio block.
#[tauri::command]
fn read_waveform(path: String) -> Result<Vec<f32>, String> {
    let target = resolve(&path)?;
    if !is_audio(&target) {
        return Err(format!("zboub69: {path} is not a supported audio file"));
    }
    let sample = zboub69::core::wav::decode(&target, usize::MAX)
        .map_err(|error| format!("zboub69: cannot read {path}: {error}"))?;
    Ok(peaks(&sample.frames, WAVEFORM_BUCKETS))
}

/// Canonicalise a path, reporting the failure as a message rather than panicking.
fn resolve(path: &str) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|error| format!("zboub69: cannot read {path}: {error}"))
}

/// Whether a path carries a supported audio file extension.
fn is_audio(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "wav"
            | "mp3"
            | "flac"
            | "m4a"
            | "aac"
            | "ogg"
            | "oga"
            | "opus"
            | "aiff"
            | "aif"
            | "caf"
            | "alac"
            | "wma"
    )
}

/// Describe one directory entry, with a zero timestamp when the filesystem does
/// not offer one.
fn describe(entry: &std::fs::DirEntry) -> SampleEntry {
    let path = entry.path();
    let meta = entry.metadata().ok();
    let directory = meta.as_ref().is_some_and(std::fs::Metadata::is_dir);
    let kind = if directory {
        SampleKind::Dir
    } else if is_audio(&path) {
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
        {
            SampleKind::Wav
        } else {
            SampleKind::Audio
        }
    } else {
        SampleKind::Other
    };
    let modified_ms = meta
        .as_ref()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_millis() as u64);
    SampleEntry {
        name: entry.file_name().to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        kind,
        size: meta.as_ref().map_or(0, std::fs::Metadata::len),
        modified_ms,
    }
}

/// Start the engine, streaming when `ZBOUB_STREAM_PORT` asks for it, and falling
/// back to silence so the window always opens.
fn start_app() -> App {
    let requested = std::env::var("ZBOUB_STREAM_PORT").ok();

    let Some(port) = stream_port(requested.as_deref()) else {
        return default_app();
    };

    match App::new_streaming(port) {
        Ok(app) => {
            eprintln!("zboub69: streaming PCM on 127.0.0.1:{port}");
            app
        }
        Err(error) => {
            eprintln!(
                "zboub69: streaming on port {port} did not start ({error:#}); using the default backend"
            );
            default_app()
        }
    }
}

/// Start the default engine, or the silent one when no device answers.
fn default_app() -> App {
    App::new().unwrap_or_else(|error| {
        eprintln!("zboub69: audio did not start ({error:#}); running silent");
        App::new_silent().expect("the silent engine always starts")
    })
}

/// Own the engine for the life of the process, in a thread it never leaves.
fn engine_thread(commands: Receiver<Event>, snapshot: Snapshot) {
    let mut app = start_app();
    let publish = |app: &App| {
        if let Ok(mut slot) = snapshot.lock() {
            *slot = Some(app.ui_state());
        }
    };
    publish(&app);

    loop {
        match commands.recv_timeout(FRAME) {
            Ok(event) => app.submit(event),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        app.tick();
        publish(&app);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let snapshot: Snapshot = Arc::new(Mutex::new(None));
    let (commands, receiver) = unbounded::<Event>();
    let published = snapshot.clone();

    tauri::Builder::default()
        .manage(snapshot)
        .manage(commands)
        .setup(move |_app| {
            std::thread::Builder::new()
                .name("engine".into())
                .spawn(move || engine_thread(receiver, published))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            send,
            samples_root,
            list_samples,
            home_dir,
            parent_of,
            read_waveform
        ])
        .run(tauri::generate_context!())
        .expect("zboub69: the tauri runtime failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nonzero_port_asks_for_the_stream() {
        assert_eq!(stream_port(Some("9999")), Some(9999));
    }

    #[test]
    fn anything_else_asks_for_the_default_backend() {
        assert_eq!(stream_port(None), None);
        assert_eq!(stream_port(Some("")), None);
        assert_eq!(stream_port(Some("0")), None);
        assert_eq!(stream_port(Some("65536")), None);
        assert_eq!(stream_port(Some("nope")), None);
    }

    #[test]
    fn audio_extensions_are_recognised_whatever_their_case() {
        assert!(is_audio(Path::new("/samples/Kick.WAV")));
        assert!(is_audio(Path::new("/samples/kick.wav")));
        assert!(is_audio(Path::new("/samples/kick.mp3")));
        assert!(is_audio(Path::new("/samples/kick.flac")));
        assert!(is_audio(Path::new("/samples/kick.m4a")));
        assert!(!is_audio(Path::new("/samples/kick.txt")));
        assert!(!is_audio(Path::new("/samples/kick")));
    }

    #[test]
    fn a_parent_is_offered_except_at_the_root() {
        assert_eq!(
            parent_of("/home/user/music".into()),
            Some("/home/user".into())
        );
        assert_eq!(parent_of("music".into()), Some("".into()));
        assert_eq!(parent_of("/".into()), None);
    }

    #[test]
    fn a_missing_directory_is_an_error_not_a_panic() {
        let listing = list_samples(Some("/nonexistent/zboub69".into()));
        assert!(
            listing.is_err(),
            "a missing directory must report, not panic"
        );
    }

    #[test]
    fn a_directory_outside_the_sample_root_is_refused() {
        let root = samples_root();
        let outside = if root.starts_with("/etc") {
            "/usr"
        } else {
            "/etc"
        };
        let listing = list_samples(Some(outside.into()));
        assert!(
            listing.is_err(),
            "the browser must not walk out of its root: {listing:?}"
        );
    }

    #[test]
    fn an_entry_crosses_ipc_as_camel_case_json() {
        let entry = SampleEntry {
            name: "kick.wav".into(),
            path: "/samples/kick.wav".into(),
            kind: SampleKind::Wav,
            size: 1_024,
            modified_ms: 42,
        };
        let json = serde_json::to_value(&entry).expect("an entry should serialize");
        assert_eq!(json["modifiedMs"], 42);
        assert_eq!(json["kind"], "wav");
        assert_eq!(json["size"], 1_024);
    }

    #[test]
    fn every_entry_kind_has_a_lowercase_name() {
        let kinds = [SampleKind::Dir, SampleKind::Wav, SampleKind::Other];
        let names: Vec<String> = kinds
            .iter()
            .map(|kind| serde_json::to_value(kind).expect("a kind should serialize"))
            .map(|value| value.as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(names, ["dir", "wav", "other"]);
    }
}
