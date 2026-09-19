//! Phase 4.1 — offline rendering and WAV bouncing.
//!
//! The global allocator is registered here so the engine's `assert_no_alloc`
//! scopes actually verify the red-zone contract during the bounce.

#[global_allocator]
static GLOBAL_ALLOC: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

use std::path::PathBuf;

use terminal_daw::core::DEFAULT_SAMPLE_RATE;
use terminal_daw::core::offline;
use terminal_daw::models::Song;

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

#[test]
fn rendered_length_matches_the_song() {
    let song = Song::demo();
    let samples = offline::render_song(&song, 2, DEFAULT_SAMPLE_RATE);
    let expected = song.frames_per_loop(DEFAULT_SAMPLE_RATE) as usize * 2 * 2;
    assert_eq!(samples.len(), expected);
}

#[test]
fn rendering_the_demo_produces_audio() {
    let song = Song::demo();
    let samples = offline::render_song(&song, 1, DEFAULT_SAMPLE_RATE);
    let peak = samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
    assert!(peak > 0.01, "expected an audible bounce, peak = {peak}");
}

#[test]
fn zero_loops_render_nothing() {
    assert!(offline::render_song(&Song::demo(), 0, DEFAULT_SAMPLE_RATE).is_empty());
}

#[test]
fn rendering_is_allocation_free() {
    let song = Song::demo();
    // `render_song` runs the DSP loop under `assert_no_alloc`, so a hidden
    // allocation would panic this test.
    let samples = offline::render_song(&song, 1, DEFAULT_SAMPLE_RATE);
    assert!(!samples.is_empty());
}

#[test]
fn export_writes_a_valid_wav_file() {
    let song = Song::demo();
    let path = temp_path("zboub69_test_export.wav");
    offline::export_song(&song, &path, 1, DEFAULT_SAMPLE_RATE).expect("export should succeed");

    let bytes = std::fs::read(&path).expect("wav should exist");
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(&bytes[36..40], b"data");

    let expected = 44 + song.frames_per_loop(DEFAULT_SAMPLE_RATE) as usize * 2 * 2;
    assert_eq!(bytes.len(), expected);

    let _ = std::fs::remove_file(&path);
}
