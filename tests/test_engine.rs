//! Integration tests for the audio engine's control surface.

use std::thread::sleep;
use std::time::{Duration, Instant};

use terminal_daw::core::engine::AudioEngine;
use terminal_daw::core::render::RtCommand;
use terminal_daw::core::sequencer::StepData;
use terminal_daw::models::Song;

#[test]
fn silent_engine_starts_cleanly() {
    let song = Song::demo();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert_eq!(engine.backend_name(), "silent");
    assert_eq!(engine.sample_rate(), 44_100);
}

#[test]
fn transport_position_advances_while_playing() {
    let song = Song::demo();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    engine.send(RtCommand::Play);

    let deadline = Instant::now() + Duration::from_secs(2);
    while engine.position_frames() == 0 && Instant::now() < deadline {
        sleep(Duration::from_millis(10));
    }
    assert!(
        engine.position_frames() > 0,
        "position should advance once playing"
    );
    assert!(engine.is_playing());
}

#[test]
fn stop_rewinds_the_transport() {
    let song = Song::demo();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    engine.send(RtCommand::Play);
    sleep(Duration::from_millis(60));
    engine.send(RtCommand::Stop);

    let deadline = Instant::now() + Duration::from_secs(2);
    while engine.position_frames() != 0 && Instant::now() < deadline {
        sleep(Duration::from_millis(10));
    }
    assert_eq!(engine.position_frames(), 0);
    assert!(!engine.is_playing());
}

#[test]
fn resync_succeeds_for_a_full_song() {
    let song = Song::demo();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert!(engine.resync(&song));
}

#[test]
fn step_edits_can_be_pushed_while_running() {
    let song = Song::new();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    let pushed = engine.send(RtCommand::SetStep {
        track: 0,
        step: 0,
        data: StepData {
            active: true,
            pitch: 60,
            velocity: 120,
        },
    });
    assert!(pushed, "command queue should accept the edit");
}
