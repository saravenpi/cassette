//! The red-zone contract, enforced in debug builds.
//!
//! `core::RenderCore::process` runs inside the audio callback, where an
//! allocation is a dropout. These tests register the checking global allocator
//! so the `assert_no_alloc` guard has something to verify against.
//!
//! `assert_no_alloc` compiles its checks out in release and does not even export
//! `AllocDisabler` there, so the whole module is debug-only.

#[cfg(debug_assertions)]
mod red_zone {
    use zboub69::core::DEFAULT_SAMPLE_RATE;
    use zboub69::core::MAX_TRACKS;
    use zboub69::core::render::{BlockName, RenderCore, RtCommand};
    use zboub69::models::Song;

    #[global_allocator]
    static GLOBAL_ALLOC: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

    #[test]
    fn the_render_loop_allocates_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::Play);
        core.apply(RtCommand::AddBlock {
            track: 0,
            name: BlockName::new("Verse"),
        });
        core.apply(RtCommand::NoteOn {
            track: 0,
            pitch: 60,
            velocity: 100,
        });

        // Everything the loop touches is allocated here, outside the guard.
        let mut out = vec![0.0f32; 1024 * 2];
        let mut peaks = [0.0f32; MAX_TRACKS];

        assert_no_alloc::assert_no_alloc(|| {
            for _ in 0..64 {
                peaks = core.process(&mut out);
            }
        });

        assert!(
            peaks.iter().any(|peak| *peak > 0.0),
            "the loop did run and produced signal"
        );
    }

    #[test]
    fn applying_commands_allocates_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        let name = BlockName::new("Chorus");

        assert_no_alloc::assert_no_alloc(|| {
            core.apply(RtCommand::SetBpm { bpm: 128.0 });
            core.apply(RtCommand::SetTrackCount { count: 4 });
            core.apply(RtCommand::AddBlock { track: 0, name });
            core.apply(RtCommand::NoteOn {
                track: 0,
                pitch: 64,
                velocity: 90,
            });
            core.apply(RtCommand::NoteOff {
                track: 0,
                pitch: 64,
            });
            core.apply(RtCommand::Stop);
        });

        assert_eq!(core.block_count(0), 1);
    }
}
