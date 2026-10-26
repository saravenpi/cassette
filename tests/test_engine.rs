//! Integration tests for the audio engine's control surface.

use std::io::Read;
use std::net::TcpStream;
use std::thread::sleep;
use std::time::{Duration, Instant};

use cassette::core::engine::AudioEngine;
use cassette::core::render::{MirroredBlock, MirroredNote, RtCommand};
use cassette::core::sample::{NO_SLOT, SampleMsg};
use cassette::models::Song;

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
    let pushed = engine.set_blocks(
        0,
        vec![MirroredBlock {
            start_beats: 0.0,
            end_beats: 4.0,
            slot: 0,
            gain: 1.0,
            offset_seconds: 0.0,
            rate: 1.0,
        }],
    );
    assert!(pushed, "command queue should accept the edit");
    assert!(engine.send(RtCommand::SetTrackCount { count: 4 }));
}

#[test]
fn a_resync_pushes_the_block_mirror() {
    let mut song = Song::new();
    song.tracks[0].add_block(0.0, 4.0);
    song.tracks[0].add_block(8.0, 4.0);
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert!(engine.resync(&song), "a resync should fit in the queue");
}

#[test]
fn the_sample_producer_is_handed_out_once() {
    let song = Song::new();
    let mut engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert!(engine.take_sample_producer().is_some());
    assert!(
        engine.take_sample_producer().is_none(),
        "the audio thread keeps the only consumer"
    );
}

/// Push a one second, constant sample into slot 0 through the ring. The
/// returned clone is the sender's, kept so the audio thread never frees it.
fn push_sample(producer: &mut rtrb::Producer<SampleMsg>) -> std::sync::Arc<[f32]> {
    let frames: std::sync::Arc<[f32]> = vec![0.5; 44_100].into();
    producer
        .push(SampleMsg::Load {
            slot: 0,
            frames: frames.clone(),
            rate: 44_100.0,
        })
        .expect("room for the load");
    frames
}

/// A block the playhead crosses straight away sounds, and its sample reaches
/// the track meter.
///
/// The block sits a hair past the top of the song because the audio side starts
/// a block only when the playhead crosses its start, and a block at beat zero
/// is behind the playhead by the time the first buffer is rendered.
#[test]
fn a_block_under_the_playhead_reaches_the_meter() {
    let song = Song::new();
    let mut engine = AudioEngine::start_silent(&song).expect("engine should start");
    let mut producer = engine.take_sample_producer().expect("a sample producer");
    let _held = push_sample(&mut producer);

    assert!(engine.set_blocks(
        0,
        vec![MirroredBlock {
            start_beats: 0.01,
            end_beats: 4.0,
            slot: 0,
            gain: 1.0,
            offset_seconds: 0.0,
            rate: 1.0,
        }],
    ));
    engine.send(RtCommand::Play);

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut peak = 0.0f32;
    while peak <= 0.01 && Instant::now() < deadline {
        sleep(Duration::from_millis(10));
        peak = engine.shared().peaks()[0];
    }
    assert!(
        peak > 0.01,
        "the sample should sound, the meter read {peak}"
    );
}

/// A block later in the song stays silent while the playhead is at the top.
#[test]
fn a_block_later_in_the_song_stays_silent() {
    let song = Song::new();
    let mut engine = AudioEngine::start_silent(&song).expect("engine should start");
    let mut producer = engine.take_sample_producer().expect("a sample producer");
    let _held = push_sample(&mut producer);

    assert!(engine.set_blocks(
        0,
        vec![MirroredBlock {
            start_beats: 64.0,
            end_beats: 68.0,
            slot: 0,
            gain: 1.0,
            offset_seconds: 0.0,
            rate: 1.0,
        }],
    ));
    engine.send(RtCommand::Play);
    sleep(Duration::from_millis(200));

    let peaks = engine.shared().peaks();
    assert_eq!(peaks[0], 0.0, "a block at beat 64 must not fire early");
    assert_eq!(peaks.iter().fold(0.0f32, |max, p| max.max(*p)), 0.0);
}

/// A note in a MIDI block's mirror sounds on its track once the playhead
/// reaches it, and not before.
#[test]
fn a_mirrored_note_sounds_at_its_start() {
    let song = Song::new();
    let engine = AudioEngine::start_silent(&song).expect("engine should start");
    assert!(engine.set_blocks(
        0,
        vec![MirroredBlock {
            start_beats: 0.0,
            end_beats: 4.0,
            slot: NO_SLOT,
            gain: 0.0,
            offset_seconds: 0.0,
            rate: 1.0,
        }],
    ));
    assert!(engine.set_notes(
        0,
        vec![MirroredNote {
            start_beats: 1.0,
            end_beats: 2.0,
            pitch: 60,
            velocity: 100,
        }],
    ));
    engine.send(RtCommand::Play);

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut peak = 0.0f32;
    while peak <= 0.01 && Instant::now() < deadline {
        sleep(Duration::from_millis(2));
        peak = engine.shared().peaks()[0];
    }
    let position = engine.position_frames();
    assert!(peak > 0.01, "the note should sound, the meter read {peak}");
    assert!(
        position >= 22_050 / 512 * 512,
        "beat one is frame 22050, yet the note sounded at frame {position}"
    );
}

/// Dropping a sample on a track makes it audible, through the app's own
/// import path: decode, allocate a slot, push the frames, mirror the block.
///
/// This is the test the whole sample feature rests on. It goes through `App`
/// rather than `AudioEngine`, so a wrong gain on the mirror or a block the
/// transport never reaches fails here and not only in the unit tests.
#[test]
fn a_dropped_sample_reaches_the_meter_end_to_end() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/cassette-mix.wav");
    let mut app = cassette::app::App::new_silent().expect("silent app should build");

    app.submit(cassette::events::Event::ImportSample {
        track: 0,
        path: path.to_string(),
        start_beats: 0.0,
    });
    let state = app.ui_state();
    let blocks = &state.tracks[0].blocks;
    assert_eq!(blocks.len(), 1, "the sample should land as one block");
    assert_eq!(blocks[0].source, path);
    assert!(
        blocks[0].length_beats > 0.0,
        "an audio block is sized from the file"
    );
    assert_ne!(
        state.tracks[0].kind,
        cassette::models::TrackKind::Midi,
        "dropping a sample makes the track an audio track"
    );

    app.submit(cassette::events::Event::TogglePlay);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut level = 0.0f32;
    while level <= 0.01 && Instant::now() < deadline {
        app.tick();
        sleep(Duration::from_millis(10));
        level = app.ui_state().tracks[0].level;
    }
    assert!(
        level > 0.01,
        "the dropped sample should be audible, the meter read {level}"
    );
}
