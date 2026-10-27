//! The control-side audio engine.
//!
//! [`AudioEngine`] owns the backend and a lock-free producer into the audio
//! thread. Every mutation is expressed as an [`RtCommand`]; the audio thread
//! owns the authoritative synthesis state and publishes a [`SharedState`]
//! snapshot back for the UI to read.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use rtrb::{Consumer, Producer, RingBuffer};

use crate::core::audio::{
    AudioBackend, BackendPreference, NullBackend, RenderFn, SharedState, select_backend,
};
use crate::core::effects::{EFFECT_RING_CAPACITY, EffectHandoff, EffectInstance};
use crate::core::render::{MirroredBlock, MirroredNote, RenderCore, RtCommand};
use crate::core::sample::{NO_SLOT, SAMPLE_RING_CAPACITY, SampleMsg};
use crate::core::synth::sample_gain;
use crate::core::{DEFAULT_SAMPLE_RATE, MAX_TRACKS, RT_COMMAND_CAPACITY};
use crate::models::{EffectChain, Song};

/// Everything the audio thread owns: the render core, the command consumer and
/// the shared snapshot it publishes. The sample ring's consumer lives inside the
/// render core, which drains it into the sample slots.
struct AudioThread {
    core: RenderCore,
    consumer: Consumer<RtCommand>,
    shared: Arc<SharedState>,
    master_level: Arc<AtomicU32>,
}

impl AudioThread {
    /// Fill `out`, taking in anything the control thread has sent first.
    ///
    /// Samples are drained before commands, so a load pushed a moment ago is
    /// audible in this callback and a block can trigger it in the same one.
    fn process(&mut self, out: &mut [f32]) {
        self.core.drain_samples();
        self.core.drain_effects();
        while let Ok(command) = self.consumer.pop() {
            self.core.apply(command);
        }
        let peaks = self.core.process(out);
        self.master_level
            .store(self.core.master_level().to_bits(), Ordering::Relaxed);
        self.shared.publish(
            self.core.position_frames(),
            self.core.is_playing(),
            self.core.is_looping(),
            self.core.count_in_frames(),
            &peaks,
        );
    }
}

struct EffectControl {
    producer: Producer<EffectHandoff>,
    return_consumer: Consumer<Box<EffectInstance>>,
    last_chains: [EffectChain; MAX_TRACKS],
    last_master_chain: EffectChain,
}

/// Handle to the running audio thread.
pub struct AudioEngine {
    backend: Box<dyn AudioBackend>,
    producer: Mutex<Producer<RtCommand>>,
    retired: Mutex<Vec<Arc<[MirroredBlock]>>>,
    retired_notes: Mutex<Vec<Arc<[MirroredNote]>>>,
    sample_producer: Option<Producer<SampleMsg>>,
    effects: Mutex<EffectControl>,
    shared: Arc<SharedState>,
    master_level: Arc<AtomicU32>,
    sample_rate: u32,
    backend_name: &'static str,
    stream_port: Option<u16>,
    note: String,
}

impl AudioEngine {
    /// Start audio for `song`, selecting the best available backend.
    pub fn start(song: &Song) -> Result<Self> {
        Self::start_preferred(song, BackendPreference::Auto)
    }

    /// Start audio for `song`, streaming raw PCM over loopback TCP.
    ///
    /// Pair with an SSH port forward to hear the session on your local machine.
    pub fn start_streaming(song: &Song, port: u16) -> Result<Self> {
        Self::start_preferred(song, BackendPreference::Stream { port })
    }

    /// Start audio for `song` with an explicit backend preference.
    pub fn start_preferred(song: &Song, preference: BackendPreference) -> Result<Self> {
        let choice = select_backend(preference);
        Self::start_with(song, choice.backend, choice.sample_rate, choice.note)
    }

    /// Start with the engine clock running but no audio output.
    pub fn start_silent(song: &Song) -> Result<Self> {
        Self::start_with(
            song,
            Box::new(NullBackend::new(DEFAULT_SAMPLE_RATE)),
            DEFAULT_SAMPLE_RATE,
            "silent engine (no device)".into(),
        )
    }

    fn start_with(
        song: &Song,
        mut backend: Box<dyn AudioBackend>,
        sample_rate: u32,
        note: String,
    ) -> Result<Self> {
        let shared = Arc::new(SharedState::new(sample_rate));
        let (producer, consumer) = RingBuffer::<RtCommand>::new(RT_COMMAND_CAPACITY);
        let (sample_producer, sample_consumer) = RingBuffer::<SampleMsg>::new(SAMPLE_RING_CAPACITY);
        let (effect_producer, effect_consumer) =
            RingBuffer::<EffectHandoff>::new(EFFECT_RING_CAPACITY);
        let (return_producer, return_consumer) =
            RingBuffer::<Box<EffectInstance>>::new(EFFECT_RING_CAPACITY);

        let mut core = RenderCore::new(song, sample_rate);
        core.set_sample_ring(sample_consumer);
        core.set_effect_rings(effect_consumer, return_producer);
        let master_level = Arc::new(AtomicU32::new(0));
        let mut thread = AudioThread {
            core,
            consumer,
            shared: shared.clone(),
            master_level: master_level.clone(),
        };
        let render: RenderFn = Box::new(move |out: &mut [f32]| thread.process(out));
        backend.start(render)?;

        let mut last_chains = [EffectChain::EMPTY; MAX_TRACKS];
        for (i, track) in song.tracks.iter().take(MAX_TRACKS).enumerate() {
            let mut chain = EffectChain::from_effects(&track.effects);
            for (idx, slot) in chain.slots[..chain.len as usize].iter_mut().enumerate() {
                if slot.id == 0 {
                    slot.id = (idx as u32 + 1) | 0x8000_0000;
                }
            }
            last_chains[i] = chain;
        }

        let mut last_master_chain = EffectChain::from_effects(&song.master_effects);
        for (idx, slot) in last_master_chain.slots[..last_master_chain.len as usize]
            .iter_mut()
            .enumerate()
        {
            if slot.id == 0 {
                slot.id = (idx as u32 + 1) | 0x8000_0000;
            }
        }

        Ok(Self {
            backend_name: backend.name(),
            stream_port: backend.stream_port(),
            backend,
            producer: Mutex::new(producer),
            retired: Mutex::new(Vec::new()),
            retired_notes: Mutex::new(Vec::new()),
            sample_producer: Some(sample_producer),
            effects: Mutex::new(EffectControl {
                producer: effect_producer,
                return_consumer,
                last_chains,
                last_master_chain,
            }),
            shared,
            master_level,
            sample_rate,
            note,
        })
    }

    /// Sample rate the engine is running at.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Name of the backend in use (`"cpal"`, `"ffplay"` or `"silent"`).
    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    /// Human-readable explanation of the backend choice.
    pub fn note(&self) -> &str {
        &self.note
    }

    /// Port being streamed on, when the streaming backend is active.
    pub fn stream_port(&self) -> Option<u16> {
        self.stream_port
    }

    /// The state snapshot published by the audio thread.
    pub fn shared(&self) -> &Arc<SharedState> {
        &self.shared
    }

    /// Push a command to the audio thread. Returns `false` if the queue is full
    /// (the caller should then [`AudioEngine::resync`]).
    /// Replace one track's insert chain. The control side owns building each
    /// effect's state, so a new effect costs the audio thread no allocation.
    pub fn set_effects(&self, track: usize, chain: &EffectChain) -> bool {
        if track >= MAX_TRACKS {
            return false;
        }
        self.collect_garbage();
        let mut effects = match self.effects.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        let mut assigned_chain = *chain;
        for (i, effect) in assigned_chain.slots[..assigned_chain.len as usize]
            .iter_mut()
            .enumerate()
        {
            if effect.id == 0 {
                effect.id = (i as u32 + 1) | 0x8000_0000;
            }
        }

        let old_chain = effects.last_chains[track];
        for effect in assigned_chain.effects() {
            let exists = old_chain
                .effects()
                .iter()
                .any(|old| old.id == effect.id && old.kind == effect.kind);
            if !exists {
                let mut inst = Box::new(EffectInstance::new(effect.kind, self.sample_rate as f32));
                inst.configure(
                    effect,
                    self.sample_rate as f32,
                    self.sample_rate as f32 * 60.0 / 120.0,
                );
                let handoff = EffectHandoff {
                    track: track as u8,
                    id: effect.id,
                    instance: inst,
                };
                if effects.producer.push(handoff).is_err() {
                    return false;
                }
            }
        }

        if !self.send_internal(RtCommand::SetEffects {
            track: track as u8,
            chain: assigned_chain,
        }) {
            return false;
        }

        effects.last_chains[track] = assigned_chain;
        drop(effects);
        self.collect_garbage();
        true
    }

    /// Replace the master bus insert chain. The control side owns building each
    /// effect's state, so a new effect costs the audio thread no allocation.
    pub fn set_master_effects(&self, chain: &EffectChain) -> bool {
        self.collect_garbage();
        let mut effects = match self.effects.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        let mut assigned_chain = *chain;
        for (i, effect) in assigned_chain.slots[..assigned_chain.len as usize]
            .iter_mut()
            .enumerate()
        {
            if effect.id == 0 {
                effect.id = (i as u32 + 1) | 0x8000_0000;
            }
        }

        let old_chain = effects.last_master_chain;
        for effect in assigned_chain.effects() {
            let exists = old_chain
                .effects()
                .iter()
                .any(|old| old.id == effect.id && old.kind == effect.kind);
            if !exists {
                let mut inst = Box::new(EffectInstance::new(effect.kind, self.sample_rate as f32));
                inst.configure(
                    effect,
                    self.sample_rate as f32,
                    self.sample_rate as f32 * 60.0 / 120.0,
                );
                let handoff = EffectHandoff {
                    track: 255,
                    id: effect.id,
                    instance: inst,
                };
                if effects.producer.push(handoff).is_err() {
                    return false;
                }
            }
        }

        if !self.send_internal(RtCommand::SetMasterEffects {
            chain: assigned_chain,
        }) {
            return false;
        }

        effects.last_master_chain = assigned_chain;
        drop(effects);
        self.collect_garbage();
        true
    }

    /// Set the master volume level.
    pub fn set_master_volume(&self, volume: f32) -> bool {
        self.send_internal(RtCommand::SetMasterVolume { volume })
    }

    /// Current smoothed master level.
    pub fn master_level(&self) -> f32 {
        f32::from_bits(self.master_level.load(Ordering::Relaxed))
    }

    /// Free any effect instances returned by the audio thread.
    pub fn collect_garbage(&self) {
        if let Ok(mut effects) = self.effects.lock() {
            while let Ok(instance) = effects.return_consumer.pop() {
                drop(instance);
            }
        }
    }

    fn send_internal(&self, command: RtCommand) -> bool {
        match self.producer.lock() {
            Ok(mut producer) => producer.push(command).is_ok(),
            Err(_) => false,
        }
    }

    pub fn send(&self, command: RtCommand) -> bool {
        self.collect_garbage();
        self.send_internal(command)
    }

    /// Take the sample ring's producer, once.
    ///
    /// Returns `None` from then on. Pushing [`SampleMsg`]s into it is how decoded
    /// frames reach the audio thread without it ever allocating; the control side
    /// owns the producer for the life of the session, and dropping it tells the
    /// audio thread the ring is closed.
    pub fn take_sample_producer(&mut self) -> Option<Producer<SampleMsg>> {
        self.sample_producer.take()
    }

    /// Replace one track's block mirror on the audio thread. Returns `false` if
    /// the command queue is full.
    ///
    /// The engine keeps a clone of each mirror until the audio thread has let go
    /// of it, so the audio thread never frees one.
    pub fn set_blocks(&self, track: usize, blocks: Vec<MirroredBlock>) -> bool {
        let blocks: Arc<[MirroredBlock]> = blocks.into();
        match self.retired.lock() {
            Ok(mut retired) => {
                retired.retain(|held| Arc::strong_count(held) > 1);
                retired.push(blocks.clone());
            }
            Err(_) => return false,
        }
        self.send(RtCommand::SetBlocks {
            track: track as u8,
            blocks,
        })
    }

    /// Replace one track's note mirror on the audio thread. Returns `false` if
    /// the command queue is full. Kept alive like [`AudioEngine::set_blocks`].
    pub fn set_notes(&self, track: usize, notes: Vec<MirroredNote>) -> bool {
        let notes: Arc<[MirroredNote]> = notes.into();
        match self.retired_notes.lock() {
            Ok(mut retired) => {
                retired.retain(|held| Arc::strong_count(held) > 1);
                retired.push(notes.clone());
            }
            Err(_) => return false,
        }
        self.send(RtCommand::SetNotes {
            track: track as u8,
            notes,
        })
    }

    pub fn delete_track(&self, track: usize) -> bool {
        if track >= MAX_TRACKS {
            return false;
        }
        if let Ok(mut effects) = self.effects.lock() {
            for i in track..(MAX_TRACKS - 1) {
                effects.last_chains[i] = effects.last_chains[i + 1];
            }
            effects.last_chains[MAX_TRACKS - 1] = EffectChain::EMPTY;
        }
        self.send(RtCommand::DeleteTrack { track: track as u8 })
    }

    pub fn move_track(&self, from: usize, to: usize) -> bool {
        if from >= MAX_TRACKS || to >= MAX_TRACKS || from == to {
            return false;
        }
        if let Ok(mut effects) = self.effects.lock() {
            if from < to {
                effects.last_chains[from..=to].rotate_left(1);
            } else {
                effects.last_chains[to..=from].rotate_right(1);
            }
        }
        self.send(RtCommand::MoveTrack {
            from: from as u8,
            to: to as u8,
        })
    }

    /// Re-send the entire song to the audio thread. Recovery path when the
    /// command queue has overflowed, and used after bulk edits.
    ///
    /// The block mirror is rebuilt from the song's blocks. Only the song is
    /// available here, so the slots are left empty: the app knows which sample
    /// each block points at and pushes the real mirror with
    /// [`AudioEngine::set_blocks`].
    pub fn resync(&self, song: &Song) -> bool {
        let mut ok = true;
        // Setting the track count also clears the audio-side block mirror.
        ok &= self.send(RtCommand::SetTrackCount {
            count: song.tracks.len().min(crate::core::MAX_TRACKS) as u8,
        });
        ok &= self.send(RtCommand::SetBpm { bpm: song.bpm() });
        ok &= self.send(RtCommand::loop_region(song.loop_region()));
        ok &= self.set_master_volume(song.master_volume);
        ok &= self.set_master_effects(&EffectChain::from_effects(&song.master_effects));
        for (track_index, track) in song.tracks.iter().enumerate() {
            let track_id = track_index as u8;
            ok &= self.send(RtCommand::SetTrackInstrument {
                track: track_id,
                instrument: track.instrument,
            });
            ok &= self.send(RtCommand::SetTrackMix {
                track: track_id,
                volume: track.volume,
                pan: track.pan,
                muted: track.muted,
                solo: track.solo,
            });
            ok &= self.set_effects(track_index, &EffectChain::from_effects(&track.effects));
            let blocks: Vec<MirroredBlock> = track
                .blocks
                .iter()
                .map(|block| MirroredBlock {
                    start_beats: block.start_beats,
                    end_beats: block.start_beats + block.length_beats,
                    slot: NO_SLOT,
                    gain: sample_gain(&track.instrument),
                    offset_seconds: block.offset_seconds,
                    rate: block.rate,
                })
                .collect();
            ok &= self.set_blocks(track_index, blocks);
        }
        ok
    }

    /// Read the transport position in frames.
    pub fn position_frames(&self) -> u64 {
        self.shared.position_frames.load(Ordering::Relaxed)
    }

    /// Whether the transport is running according to the audio thread.
    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }

    /// Frames of count-in pre-roll still to run, according to the audio thread.
    pub fn count_in_frames(&self) -> u64 {
        self.shared.count_in_frames.load(Ordering::Relaxed)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.backend.stop();
        self.collect_garbage();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Song;

    #[test]
    fn engine_starts_and_stops() {
        let song = Song::new();
        let engine = AudioEngine::start(&song).expect("engine should start");
        assert!(engine.sample_rate() > 0);
        assert!(!engine.backend_name().is_empty());
        drop(engine);
    }

    #[test]
    fn resync_pushes_the_whole_song() {
        let song = Song::new();
        let engine = AudioEngine::start(&song).expect("engine should start");
        assert!(engine.resync(&song));
    }

    #[test]
    fn engine_handles_master_bus_commands() {
        let song = Song::new();
        let engine = AudioEngine::start_silent(&song).expect("engine should start");
        assert!(engine.set_master_volume(0.8));
        let chain = EffectChain::from_effects(&[crate::models::Effect::new(
            crate::models::EffectKind::Limiter,
        )]);
        assert!(engine.set_master_effects(&chain));
        assert!(engine.master_level() >= 0.0);
    }

    #[test]
    fn streaming_engine_uses_the_stream_backend() {
        let song = Song::new();
        // Port 0 lets the OS pick, so parallel tests can never clash.
        let engine = AudioEngine::start_streaming(&song, 0).expect("engine should start");
        assert_eq!(engine.backend_name(), "stream");
        assert!(engine.stream_port().is_some());
        assert!(engine.note().contains("streaming"));
    }
}
