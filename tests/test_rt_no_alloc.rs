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
    use cassette::core::DEFAULT_SAMPLE_RATE;
    use cassette::core::MAX_TRACKS;
    use cassette::core::render::{MirroredBlock, RenderCore, RtCommand};
    use cassette::core::sample::{NO_SLOT, SampleMsg};
    use cassette::models::Song;
    use std::sync::Arc;

    #[global_allocator]
    static GLOBAL_ALLOC: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

    /// One MIDI lane's worth of mirror, built before the guard goes up.
    fn lane() -> Arc<[MirroredBlock]> {
        Arc::from(vec![
            MirroredBlock {
                start_beats: 0.0,
                end_beats: 4.0,
                slot: NO_SLOT,
                gain: 0.0,
            },
            MirroredBlock {
                start_beats: 8.0,
                end_beats: 12.0,
                slot: NO_SLOT,
                gain: 0.0,
            },
        ])
    }

    #[test]
    fn the_render_loop_allocates_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::Play);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: lane(),
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
        let blocks = lane();

        assert_no_alloc::assert_no_alloc(|| {
            core.apply(RtCommand::SetBpm { bpm: 128.0 });
            core.apply(RtCommand::SetLooping { looping: true });
            core.apply(RtCommand::SetSampleRate {
                slot: 0,
                rate: 44_100.0,
            });
            core.apply(RtCommand::SetBlocks {
                track: 0,
                blocks: blocks.clone(),
            });
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

        assert_eq!(core.block_count(0), 2);
    }

    #[test]
    fn replacing_a_non_empty_mirror_allocates_and_frees_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: lane(),
        });
        let keep = lane();
        let replacement: Arc<[MirroredBlock]> = Arc::from(vec![MirroredBlock {
            start_beats: 16.0,
            end_beats: 20.0,
            slot: NO_SLOT,
            gain: 0.0,
        }]);
        let command = RtCommand::SetBlocks {
            track: 0,
            blocks: replacement.clone(),
        };

        assert_no_alloc::assert_no_alloc(|| {
            core.apply(command);
            core.apply(RtCommand::SetBlocks {
                track: 0,
                blocks: keep.clone(),
            });
            core.apply(RtCommand::Seek { beats: 1.0 });
        });

        assert_eq!(core.block_count(0), 2);
        assert_eq!(replacement.len(), 1);
    }

    /// Fill one slot with `frames` frames of a constant value.
    fn load_slot(core: &mut RenderCore, slot: u8, frames: usize) {
        core.push_sample(SampleMsg::Begin {
            slot,
            frames: frames as u32,
            rate: 44_100.0,
        });
        for _ in 0..frames {
            core.push_sample(SampleMsg::Frame(0.5));
        }
        core.push_sample(SampleMsg::End { slot });
    }

    #[test]
    fn playing_a_mirrored_sample_allocates_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        load_slot(&mut core, 0, 4_096);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: Arc::from(vec![MirroredBlock {
                start_beats: 0.0,
                end_beats: 4.0,
                slot: 0,
                gain: 0.5,
            }]),
        });
        core.apply(RtCommand::Play);

        let mut out = vec![0.0f32; 1024 * 2];

        assert_no_alloc::assert_no_alloc(|| {
            for _ in 0..64 {
                let _ = core.process(&mut out);
            }
        });

        assert!(core.position_frames() > 0, "the loop did run");
    }
}
