//! The cassette desktop shell: a thin Tauri command surface over the engine.
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

use cassette::app::{App, WAVEFORM_BUCKETS, peaks};
use cassette::core::wav::MAX_DECODED_FRAMES;
use cassette::events::Event;
use cassette::ui::view::UiState;

/// One frame at 60 Hz, the fastest the webview can draw.
const FRAME: Duration = Duration::from_micros(16_667);
/// Entries one directory listing returns at most.
const MAX_ENTRIES: usize = 2_000;

/// The latest snapshot, published by the engine thread.
type Snapshot = Arc<Mutex<Option<UiState>>>;
/// Intent from the webview, drained by the engine thread.
type Commands = Sender<Event>;

/// The current snapshot, or `None` in the moment before the engine first publishes.
///
/// `known` holds the notes revision the webview has for each track; those
/// tracks come back without notes, so an idle frame carries none.
#[tauri::command]
fn state(known: Vec<u64>, snapshot: State<'_, Snapshot>) -> Result<Option<UiState>, String> {
    let snapshot = snapshot
        .lock()
        .map_err(|error| format!("cassette: {error}"))?;
    Ok(snapshot
        .clone()
        .map(|state| state.without_known_notes(&known)))
}

/// Apply one intent. Resolves as soon as the engine thread has taken it.
#[tauri::command]
fn send(event: Event, commands: State<'_, Commands>) -> Result<(), String> {
    commands
        .send(event)
        .map_err(|_| "cassette: the engine thread has stopped".to_string())
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
    /// A project file the engine can load.
    Project,
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

/// The directory the sample browser opens on: `$CASSETTE_SAMPLES_DIR` or `$ZBOUB_SAMPLES_DIR` when set,
/// else `~/Music` when it exists, else the home directory.
#[tauri::command]
fn samples_root() -> String {
    if let Ok(dir) =
        std::env::var("CASSETTE_SAMPLES_DIR").or_else(|_| std::env::var("ZBOUB_SAMPLES_DIR"))
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
async fn list_samples(dir: Option<String>) -> Result<Vec<SampleEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || list_dir(dir))
        .await
        .map_err(|error| format!("cassette: {error}"))?
}

/// The blocking body of [`list_samples`]. When a directory holds more than
/// [`MAX_ENTRIES`] rows, the list ends with a notice row of kind `other`.
fn list_dir(dir: Option<String>) -> Result<Vec<SampleEntry>, String> {
    let root = resolve(&samples_root())?;
    let requested = dir.unwrap_or_else(|| root.to_string_lossy().into_owned());
    let target = resolve(&requested)?;
    if !target.starts_with(&root) {
        return Err(format!(
            "cassette: {requested} is outside the sample root {}",
            root.display()
        ));
    }
    let listing = std::fs::read_dir(&target)
        .map_err(|error| format!("cassette: cannot read {}: {error}", target.display()))?;
    let entries = listing
        .filter_map(|entry| entry.ok())
        .map(|entry| describe(&entry))
        .collect();
    Ok(sorted_and_capped(entries, MAX_ENTRIES))
}

/// Directories first, then names; past `cap` rows the tail becomes one notice row.
fn sorted_and_capped(mut entries: Vec<SampleEntry>, cap: usize) -> Vec<SampleEntry> {
    entries.sort_by_key(|entry| (entry.kind != SampleKind::Dir, entry.name.to_lowercase()));
    if entries.len() > cap {
        let hidden = entries.len() - cap;
        entries.truncate(cap);
        entries.push(SampleEntry {
            name: format!("{hidden} more entries not shown"),
            path: String::new(),
            kind: SampleKind::Other,
            size: 0,
            modified_ms: 0,
        });
    }
    entries
}

/// The drawing of one audio file: its peaks and how long the file runs, so a
/// block can show only the stretch of the file it plays.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Waveform {
    peaks: Vec<f32>,
    seconds: f32,
}

/// Peaks of an audio file for drawing an audio block. Any readable path is
/// accepted, so files dropped from outside the sample root still draw; the
/// decode stops at [`MAX_DECODED_FRAMES`].
#[tauri::command]
async fn read_waveform(path: String) -> Result<Waveform, String> {
    tauri::async_runtime::spawn_blocking(move || waveform_of(&path))
        .await
        .map_err(|error| format!("cassette: {error}"))?
}

/// The blocking body of [`read_waveform`].
fn waveform_of(path: &str) -> Result<Waveform, String> {
    let target = resolve(path)?;
    if !is_audio(&target) {
        return Err(format!("cassette: {path} is not a supported audio file"));
    }
    let sample = cassette::core::wav::decode(&target, MAX_DECODED_FRAMES)
        .map_err(|error| format!("cassette: cannot read {path}: {error:#}"))?;
    Ok(Waveform {
        peaks: peaks(&sample.frames, WAVEFORM_BUCKETS),
        seconds: sample.frames.len() as f32 / sample.sample_rate.max(1) as f32,
    })
}

/// Canonicalise a path, reporting the failure as a message rather than panicking.
fn resolve(path: &str) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|error| format!("cassette: cannot read {path}: {error}"))
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
    let meta = std::fs::metadata(&path).ok();
    let directory = meta.as_ref().is_some_and(std::fs::Metadata::is_dir);
    let kind = if directory {
        SampleKind::Dir
    } else if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        SampleKind::Project
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

/// Start the engine, streaming when `CASSETTE_STREAM_PORT` or `ZBOUB_STREAM_PORT` asks for it, and falling
/// back to silence so the window always opens.
fn start_app() -> App {
    let requested = std::env::var("CASSETTE_STREAM_PORT")
        .ok()
        .or_else(|| std::env::var("ZBOUB_STREAM_PORT").ok());

    let Some(port) = stream_port(requested.as_deref()) else {
        return default_app();
    };

    match App::new_streaming(port) {
        Ok(app) => {
            eprintln!("cassette: streaming PCM on 127.0.0.1:{port}");
            app
        }
        Err(error) => {
            eprintln!(
                "cassette: streaming on port {port} did not start ({error:#}); using the default backend"
            );
            default_app()
        }
    }
}

/// Start the default engine, or the silent one when no device answers.
fn default_app() -> App {
    App::new().unwrap_or_else(|error| {
        eprintln!("cassette: audio did not start ({error:#}); running silent");
        App::new_silent().expect("the silent engine always starts")
    })
}

/// Own the engine for the life of the process, in a thread it never leaves.
fn engine_thread(commands: Receiver<Event>, snapshot: Snapshot) {
    let mut app = start_app();
    let published = snapshot.clone();
    app.set_progress_reporter(move |progress| {
        if let Ok(mut slot) = published.lock() {
            if let Some(ref mut state) = *slot {
                state.export_progress = progress;
            }
        }
    });
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
        .plugin(tauri_plugin_dialog::init())
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
        .expect("cassette: the tauri runtime failed");
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
        let listing = list_dir(Some("/nonexistent/cassette".into()));
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
        let listing = list_dir(Some(outside.into()));
        assert!(
            listing.is_err(),
            "the browser must not walk out of its root: {listing:?}"
        );
    }

    fn entry(name: &str, kind: SampleKind) -> SampleEntry {
        SampleEntry {
            name: name.into(),
            path: format!("/s/{name}"),
            kind,
            size: 0,
            modified_ms: 0,
        }
    }

    #[test]
    fn the_cap_keeps_the_sorted_head_and_flags_the_rest() {
        let entries = vec![
            entry("z.wav", SampleKind::Wav),
            entry("b.wav", SampleKind::Wav),
            entry("sub", SampleKind::Dir),
            entry("a.wav", SampleKind::Wav),
        ];
        let capped = sorted_and_capped(entries, 2);
        let names: Vec<&str> = capped.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["sub", "a.wav", "2 more entries not shown"]);
        assert_eq!(capped[2].kind, SampleKind::Other);
        assert!(capped[2].path.is_empty());
    }

    #[test]
    fn a_listing_within_the_cap_gets_no_notice() {
        let capped = sorted_and_capped(vec![entry("a.wav", SampleKind::Wav)], 1);
        assert_eq!(capped.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_described_by_its_target() {
        let dir = std::env::temp_dir().join("cassette-describe-symlink");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("real")).expect("dir");
        std::os::unix::fs::symlink(dir.join("real"), dir.join("link")).expect("symlink");
        let found = std::fs::read_dir(&dir)
            .expect("listing")
            .filter_map(Result::ok)
            .find(|e| e.file_name() == "link")
            .map(|e| describe(&e));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found.expect("the link is listed").kind, SampleKind::Dir);
    }

    #[test]
    fn a_waveform_outside_the_sample_root_still_reads() {
        let path = std::env::temp_dir().join("cassette-waveform-outside.wav");
        let mut data = Vec::new();
        for value in [0i16, 16_384, -16_384, 0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&88_200u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        std::fs::write(&path, wav).expect("write");
        let waveform = waveform_of(&path.to_string_lossy());
        let _ = std::fs::remove_file(&path);
        let waveform = waveform.expect("should read");
        assert_eq!(waveform.peaks.len(), WAVEFORM_BUCKETS);
        assert!(waveform.seconds > 0.0);
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
        let kinds = [
            SampleKind::Dir,
            SampleKind::Wav,
            SampleKind::Audio,
            SampleKind::Project,
            SampleKind::Other,
        ];
        let names: Vec<String> = kinds
            .iter()
            .map(|kind| serde_json::to_value(kind).expect("a kind should serialize"))
            .map(|value| value.as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(names, ["dir", "wav", "audio", "project", "other"]);
    }
}
