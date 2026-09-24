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

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam::channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use tauri::State;

use zboub69::app::App;
use zboub69::events::Event;
use zboub69::ui::view::UiState;

/// One frame at 60 Hz, the fastest the webview can draw.
const FRAME: Duration = Duration::from_micros(16_667);

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

/// Start the engine, falling back to silence so the window always opens.
fn start_app() -> App {
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
        .invoke_handler(tauri::generate_handler![state, send])
        .run(tauri::generate_context!())
        .expect("zboub69: the tauri runtime failed");
}
