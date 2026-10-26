//! The control-side audio engine.
//!
//! [`AudioEngine`] owns the backend and a lock-free producer into the audio
//! thread. Every mutation is expressed as an [`RtCommand`]; the audio thread
//! owns the authoritative synthesis state and publishes a [`SharedState`]
//! snapshot back for the UI to read.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use anyhow::Result;
use rtrb::{Consumer, Producer, RingBuffer};

use crate::core::audio::{
    AudioBackend, BackendPreference, NullBackend, RenderFn, SharedState, select_backend,
};
use crate::core::render::{MirroredBlock, MirroredNote, RenderCore, RtCommand};
use crate::core::sample::{NO_SLOT, SAMPLE_RING_CAPACITY, SampleMsg};
use crate::core::synth::sample_gain;
use crate::core::{DEFAULT_SAMPLE_RATE, RT_COMMAND_CAPACITY};
use crate::models::Song;

/// Everything the audio thread owns: the render core, the command consumer and
/// the shared snapshot it publishes. The sample ring's consumer lives inside the
/// render core, which drains it into the sample slots.
struct AudioThread {
    core: RenderCore,
    consumer: Consumer<RtCommand>,
    shared: Arc<SharedState>,
}

impl AudioThread {
    /// Fill `out`, taking in anything the control thread has sent first.
    ///
    /// Samples are drained before commands, so a load pushed a moment ago is
    /// audible in this callback and a block can trigger it in the same one.
    fn process(&mut self, out: &mut [f32]) {
        self.core.drain_samples();
        while let Ok(command) = self.consumer.pop() {
            self.core.apply(command);
        }
        let peaks = self.core.process(out);
        self.shared.publish(
            self.core.position_frames(),
            self.core.is_playing(),
            self.core.is_looping(),
            &peaks,
        );
    }
}

/// Handle to the running audio thread.
pub struct AudioEngine {
    backend: Box<dyn AudioBackend>,
    producer: Mutex<Producer<RtCommand>>,
    retired: Mutex<Vec<Arc<[MirroredBlock]>>>,
    retired_notes: Mutex<Vec<Arc<[MirroredNote]>>>,
    sample_producer: Option<Producer<SampleMsg>>,
    shared: Arc<SharedState>,
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

        let mut core = RenderCore::new(song, sample_rate);
        core.set_sample_ring(sample_consumer);
        let mut thread = AudioThread {
            core,
            consumer,
            shared: shared.clone(),
        };
        let render: RenderFn = Box::new(move |out: &mut [f32]| thread.process(out));
        backend.start(render)?;

        Ok(Self {
            backend_name: backend.name(),
            stream_port: backend.stream_port(),
            backend,
            producer: Mutex::new(producer),
            retired: Mutex::new(Vec::new()),
            retired_notes: Mutex::new(Vec::new()),
            sample_producer: Some(sample_producer),
            shared,
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
    pub fn send(&self, command: RtCommand) -> bool {
        match self.producer.lock() {
            Ok(mut producer) => producer.push(command).is_ok(),
            Err(_) => false,
        }
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
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.backend.stop();
        // Dropping the producer disconnects the consumer, ending the audio loop.
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
    fn streaming_engine_uses_the_stream_backend() {
        let song = Song::new();
        // Port 0 lets the OS pick, so parallel tests can never clash.
        let engine = AudioEngine::start_streaming(&song, 0).expect("engine should start");
        assert_eq!(engine.backend_name(), "stream");
        assert!(engine.stream_port().is_some());
        assert!(engine.note().contains("streaming"));
    }
}
