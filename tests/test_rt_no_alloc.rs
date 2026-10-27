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
    use cassette::core::render::{MirroredBlock, MirroredNote, RenderCore, RtCommand};
    use cassette::core::sample::{NO_SLOT, SampleMsg};
    use cassette::models::{Effect, EffectChain, EffectKind, Song};
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
                offset_seconds: 0.0,
                rate: 1.0,
            },
            MirroredBlock {
                start_beats: 8.0,
                end_beats: 12.0,
                slot: NO_SLOT,
                gain: 0.0,
                offset_seconds: 0.0,
                rate: 1.0,
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
            offset_seconds: 0.0,
            rate: 1.0,
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

    #[test]
    fn mirroring_and_playing_notes_allocates_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: lane(),
        });
        let notes: Arc<[MirroredNote]> = (0..32)
            .map(|i| MirroredNote {
                start_beats: i as f32 * 0.125,
                end_beats: i as f32 * 0.125 + 0.5,
                pitch: 48 + i as u8,
                velocity: 100,
            })
            .collect();
        let keep = notes.clone();
        let mut out = vec![0.0f32; 1024 * 2];
        let mut loudest = 0.0f32;

        assert_no_alloc::assert_no_alloc(|| {
            core.apply(RtCommand::SetNotes { track: 0, notes });
            core.apply(RtCommand::Play);
            for _ in 0..64 {
                loudest = loudest.max(core.process(&mut out)[0]);
            }
            core.apply(RtCommand::Stop);
        });

        assert_eq!(core.note_count(0), keep.len());
        assert!(loudest > 0.0, "the mirrored notes played");
    }

    /// Fill one slot with `frames` frames of a constant value. The returned
    /// clone is the sender's, so the audio side never frees the buffer.
    fn load_slot(core: &mut RenderCore, slot: u8, frames: usize) -> Arc<[f32]> {
        let held: Arc<[f32]> = vec![0.5; frames].into();
        core.push_sample(SampleMsg::Load {
            slot,
            frames: held.clone(),
            rate: 44_100.0,
        });
        held
    }

    #[test]
    fn playing_a_mirrored_sample_allocates_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        let _held = load_slot(&mut core, 0, 4_096);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: Arc::from(vec![MirroredBlock {
                start_beats: 0.0,
                end_beats: 4.0,
                slot: 0,
                gain: 0.5,
                offset_seconds: 0.0,
                rate: 1.0,
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

    /// Eight effects including duplicates and a sidechain.
    fn eight_effects() -> EffectChain {
        let mut effects = [
            Effect::new(EffectKind::Delay),
            Effect::new(EffectKind::Delay),
            Effect::new(EffectKind::Filter),
            Effect::new(EffectKind::Filter),
            Effect::new(EffectKind::Reverb),
            Effect::new(EffectKind::Eq),
            Effect::new(EffectKind::Saturator),
            Effect::new(EffectKind::Compressor),
        ];
        for (i, effect) in effects.iter_mut().enumerate() {
            effect.id = (i + 1) as u32;
        }
        effects[7].sidechain = Some(1);
        EffectChain::from_effects(&effects)
    }

    #[test]
    fn every_effect_on_a_track_allocates_nothing() {
        let song = Song::new();
        let mut core = RenderCore::new(&song, DEFAULT_SAMPLE_RATE);
        let (mut handoff_prod, handoff_cons) =
            rtrb::RingBuffer::<cassette::core::effects::EffectHandoff>::new(256);
        let (return_prod, mut _return_cons) =
            rtrb::RingBuffer::<Box<cassette::core::effects::EffectInstance>>::new(256);
        core.set_effect_rings(handoff_cons, return_prod);
        core.apply(RtCommand::SetBlocks {
            track: 0,
            blocks: lane(),
        });
        let chain = eight_effects();
        for effect in chain.effects() {
            let instance = Box::new(cassette::core::effects::EffectInstance::new(
                effect.kind,
                DEFAULT_SAMPLE_RATE as f32,
            ));
            handoff_prod
                .push(cassette::core::effects::EffectHandoff {
                    track: 0,
                    id: effect.id,
                    instance,
                })
                .expect("handoff push");
        }
        let mut moved = chain;
        moved.slots.reverse();
        moved.slots[0].params[1] = 0.9;
        let mut bypassed = moved;
        bypassed.slots[2].enabled = false;
        let mut out = vec![0.0f32; 1024 * 2];
        let mut loudest = 0.0f32;

        assert_no_alloc::assert_no_alloc(|| {
            core.drain_effects();
            core.apply(RtCommand::SetEffects { track: 0, chain });
            core.apply(RtCommand::Play);
            core.apply(RtCommand::NoteOn {
                track: 0,
                pitch: 60,
                velocity: 100,
            });
            for block in 0..64 {
                if block == 16 {
                    core.apply(RtCommand::SetEffects {
                        track: 0,
                        chain: moved,
                    });
                }
                if block == 32 {
                    core.apply(RtCommand::SetEffects {
                        track: 0,
                        chain: bypassed,
                    });
                    core.apply(RtCommand::NoteOff {
                        track: 0,
                        pitch: 60,
                    });
                }
                loudest = loudest.max(core.process(&mut out)[0]);
            }
            core.apply(RtCommand::SetEffects {
                track: 0,
                chain: EffectChain::EMPTY,
            });
        });

        assert!(
            out.iter().all(|sample| sample.is_finite()),
            "the chain stayed finite"
        );
        assert!(
            loudest > 0.0,
            "the note reached the meter through the chain"
        );
    }
}
