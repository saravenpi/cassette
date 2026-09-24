//! Integration tests for the audio engine's control surface.

use std::io::Read;
use std::net::TcpStream;
use std::thread::sleep;
use std::time::{Duration, Instant};

use terminal_daw::core::engine::AudioEngine;
use terminal_daw::core::render::RtCommand;
use terminal_daw::models::Song;

#[test]
fn silent_engine_starts_cleanly() {
    let song = Song::new();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert_eq!(engine.backend_name(), "silent");
    assert_eq!(engine.sample_rate(), 44_100);
}

#[test]
fn transport_position_advances_while_playing() {
    let song = Song::new();
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
    let song = Song::new();
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
    let song = Song::new();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert!(engine.resync(&song));
}

#[test]
fn streaming_only_renders_once_a_client_connects() {
    let song = Song::new();
    let engine = AudioEngine::start_streaming(&song, 0).expect("engine should start");
    let port = engine.stream_port().expect("a stream port");
    engine.send(RtCommand::Play);

    // With nobody listening there is no sink to pace against, so the
    // transport must not run away on its own.
    sleep(Duration::from_millis(150));
    assert_eq!(engine.position_frames(), 0, "no client, no rendering");

    let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect to the stream");
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout");

    let mut buffer = vec![0u8; 32 * 1024];
    let started = Instant::now();
    let deadline = started + Duration::from_millis(800);
    while Instant::now() < deadline {
        if client.read(&mut buffer).is_err() {
            break;
        }
    }
    let elapsed = started.elapsed().as_secs_f64();
    let position = engine.position_frames();

    assert!(
        position > 0,
        "transport should advance while a client reads the stream"
    );
    // A fast reader must not drag the transport past real time. Allow 2x for
    // scheduling jitter; the unfixed bug ran ~26x.
    let ceiling = (elapsed * 44_100.0 * 2.0) as u64;
    assert!(
        position <= ceiling,
        "transport ran ahead of real time: {position} frames in {elapsed:.2}s (ceiling {ceiling})"
    );
}

#[test]
fn block_edits_can_be_pushed_while_running() {
    let song = Song::new();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    let pushed = engine.send(RtCommand::AddBlock {
        track: 0,
        name: terminal_daw::core::render::BlockName::new("Verse"),
    });
    assert!(pushed, "command queue should accept the edit");
    assert!(engine.send(RtCommand::SetTrackCount { count: 4 }));
}

#[test]
fn a_resync_pushes_the_block_mirror() {
    let mut song = Song::new();
    song.tracks[0].add_block();
    song.tracks[0].add_block();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert!(engine.resync(&song), "a resync should fit in the queue");
}
