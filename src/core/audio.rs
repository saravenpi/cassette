//! Audio backends and the lock-free state they publish back to the UI.
//!
//! Three backends ship by default:
//!
//! * [`SubprocessBackend`] pipes raw `f32` PCM into `ffplay` — works anywhere
//!   ffmpeg is installed, no development headers required. It probes the device
//!   first, so a machine without a usable output falls back instead of hanging.
//! * [`NullBackend`] runs the engine on a paced thread and makes no sound.
//!   Useful on headless machines and in tests.
//! * `CpalBackend` (feature `cpal`) drives the device directly for the lowest
//!   latency. It is selected first when the feature is enabled.

use std::io::Write;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::core::{DEFAULT_SAMPLE_RATE, MAX_TRACKS};

/// Frames per render call for backends that pace themselves in software.
const CHUNK_FRAMES: usize = 512;
/// Frames of silence used to probe whether a player can open a device.
const PROBE_FRAMES: usize = 4096;

/// Wall-clock duration of one render chunk.
fn chunk_period(sample_rate: u32) -> Duration {
    Duration::from_secs_f64(CHUNK_FRAMES as f64 / sample_rate.max(1) as f64)
}

/// Sleep until the next chunk is due.
///
/// Blocking writes already stop us running *ahead* of a slow consumer; this
/// stops us running ahead of a fast one (a socket reader that drains instantly,
/// or a pipe into `cat`), which would otherwise race the transport.
fn pace(deadline: &mut Instant, period: Duration) {
    *deadline += period;
    let now = Instant::now();
    if *deadline > now {
        thread::sleep(*deadline - now);
    } else {
        // A blocking write stalled us; resync rather than burst to catch up.
        *deadline = now;
    }
}

/// Callback the backend calls to fill interleaved stereo samples.
pub type RenderFn = Box<dyn FnMut(&mut [f32]) + Send + 'static>;

/// A sink for rendered audio.
pub trait AudioBackend {
    /// Short name shown in the UI, e.g. `"cpal"`.
    fn name(&self) -> &'static str;
    /// The sample rate the backend is running at.
    fn sample_rate(&self) -> u32;
    /// Begin rendering. The closure is owned by the backend from here on.
    fn start(&mut self, render: RenderFn) -> Result<()>;
    /// Stop rendering and release the device.
    fn stop(&mut self);
    /// The TCP port being streamed on, for backends that stream.
    fn stream_port(&self) -> Option<u16> {
        None
    }
}

/// Transport and meter state published by the audio thread, read by the UI.
///
/// Every field is an atomic, so there is no lock on either side.
pub struct SharedState {
    pub sample_rate: u32,
    pub position_frames: AtomicU64,
    pub playing: AtomicBool,
    pub looping: AtomicBool,
    pub step_index: AtomicU32,
    /// Per-track peak level, stored as raw `f32` bits.
    pub peaks: [AtomicU32; MAX_TRACKS],
}

impl SharedState {
    /// Create a fresh shared state, transport stopped.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            position_frames: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            looping: AtomicBool::new(true),
            step_index: AtomicU32::new(0),
            peaks: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    /// Publish the audio thread's latest state.
    pub fn publish(
        &self,
        position_frames: u64,
        playing: bool,
        looping: bool,
        step_index: u32,
        peaks: &[f32; MAX_TRACKS],
    ) {
        self.position_frames
            .store(position_frames, Ordering::Relaxed);
        self.playing.store(playing, Ordering::Relaxed);
        self.looping.store(looping, Ordering::Relaxed);
        self.step_index.store(step_index, Ordering::Relaxed);
        for (slot, peak) in self.peaks.iter().zip(peaks.iter()) {
            slot.store(peak.to_bits(), Ordering::Relaxed);
        }
    }

    /// Snapshot the per-track peaks.
    pub fn peaks(&self) -> [f32; MAX_TRACKS] {
        std::array::from_fn(|i| f32::from_bits(self.peaks[i].load(Ordering::Relaxed)))
    }
}

// ---------------------------------------------------------------------------
// Null backend
// ---------------------------------------------------------------------------

/// Renders on a paced thread and discards the output.
///
/// Keeps the transport and meters alive without any audio device.
pub struct NullBackend {
    sample_rate: u32,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl NullBackend {
    /// Create a null backend at the given sample rate.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }
}

impl AudioBackend for NullBackend {
    fn name(&self) -> &'static str {
        "silent"
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn start(&mut self, mut render: RenderFn) -> Result<()> {
        let stop = self.stop.clone();
        let sample_rate = self.sample_rate;
        self.handle = Some(
            thread::Builder::new()
                .name("zboub-audio-null".into())
                .spawn(move || {
                    let mut buffer = vec![0.0f32; CHUNK_FRAMES * 2];
                    let period = chunk_period(sample_rate);
                    let mut deadline = Instant::now();
                    while !stop.load(Ordering::Relaxed) {
                        render(&mut buffer);
                        pace(&mut deadline, period);
                    }
                })
                .map_err(|e| anyhow!("failed to spawn audio thread: {e}"))?,
        );
        Ok(())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Subprocess backend (ffplay)
// ---------------------------------------------------------------------------

/// Arguments that make `ffplay` play raw stereo `f32` from stdin.
///
/// `-ch_layout` is the ffmpeg 6+ spelling; the older `-ac` is not accepted by
/// `ffplay` and causes it to exit before reading a single sample.
fn player_args(sample_rate: u32) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nodisp",
        "-autoexit",
        "-fflags",
        "nobuffer",
        "-f",
        "f32le",
        "-ar",
        &sample_rate.to_string(),
        "-ch_layout",
        "stereo",
        "-i",
        "pipe:0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Streams raw `f32` PCM into `ffplay` through a pipe.
pub struct SubprocessBackend {
    sample_rate: u32,
    program: &'static str,
    child: Option<Child>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl SubprocessBackend {
    /// Probe for a usable player *and* a usable audio device.
    pub fn new(sample_rate: u32) -> Result<Self> {
        let program: &'static str = "ffplay";
        if !program_available(program) {
            return Err(anyhow!("{program} is not installed"));
        }
        probe_device(program, sample_rate)?;
        Ok(Self {
            sample_rate,
            program,
            child: None,
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        })
    }
}

/// Whether `program` can be executed.
fn program_available(program: &str) -> bool {
    Command::new(program)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Feed a short burst of silence to `ffplay` and inspect what it says.
///
/// Returns `Ok(())` only if the player exited without complaining about audio
/// output. ALSA/SDL write their errors straight to stderr, so we can detect
/// "there is no sound card" and fall back instead of blocking on a dead pipe.
fn probe_device(program: &str, sample_rate: u32) -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    let program_owned = program.to_string();
    thread::Builder::new()
        .name("zboub-audio-probe".into())
        .spawn(move || {
            let _ = sender.send(run_probe(&program_owned, sample_rate));
        })
        .map_err(|e| anyhow!("failed to spawn probe: {e}"))?;

    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => result,
        Err(_) => Err(anyhow!("{program} did not respond within 5s")),
    }
}

fn run_probe(program: &str, sample_rate: u32) -> Result<()> {
    let mut child = Command::new(program)
        .args(player_args(sample_rate))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("failed to start {program}: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        // Under the 64 KiB pipe buffer, so a live player never blocks here.
        let silence = vec![0u8; PROBE_FRAMES * 2 * size_of::<f32>()];
        let _ = stdin.write_all(&silence);
    }

    let output = child
        .wait_with_output()
        .map_err(|e| anyhow!("{program} probe failed: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("Couldn't open audio device")
        || stderr.contains("Unknown PCM")
        || stderr.contains("cannot find card")
        || stderr.contains("No such file or directory")
    {
        return Err(anyhow!("no usable audio device: {}", first_line(&stderr)));
    }
    if !output.status.success() && !stderr.trim().is_empty() {
        return Err(anyhow!("{program}: {}", first_line(&stderr)));
    }
    Ok(())
}

/// First meaningful line of a diagnostic string.
fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown error")
        .to_string()
}

impl AudioBackend for SubprocessBackend {
    fn name(&self) -> &'static str {
        self.program
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn start(&mut self, mut render: RenderFn) -> Result<()> {
        let mut child = Command::new(self.program)
            .args(player_args(self.sample_rate))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("failed to start {}: {e}", self.program))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("{} stdin unavailable", self.program))?;
        self.child = Some(child);

        let stop = self.stop.clone();
        let sample_rate = self.sample_rate;
        self.handle = Some(
            thread::Builder::new()
                .name("zboub-audio-pipe".into())
                .spawn(move || {
                    let mut buffer = vec![0.0f32; CHUNK_FRAMES * 2];
                    let mut bytes = Vec::with_capacity(CHUNK_FRAMES * 2 * 4);
                    let period = chunk_period(sample_rate);
                    let mut deadline = Instant::now();
                    while !stop.load(Ordering::Relaxed) {
                        render(&mut buffer);
                        bytes.clear();
                        for sample in &buffer {
                            bytes.extend_from_slice(&sample.to_le_bytes());
                        }
                        // The write blocks if the player is behind us.
                        if stdin.write_all(&bytes).is_err() {
                            break;
                        }
                        pace(&mut deadline, period);
                    }
                })
                .map_err(|e| anyhow!("failed to spawn audio thread: {e}"))?,
        );
        Ok(())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Closing stdin unblocks a writer parked in `write_all`.
        if let Some(child) = self.child.as_mut() {
            let _ = child.stdin.take();
            let _ = child.kill();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
    }
}

// ---------------------------------------------------------------------------
// TCP stream backend (audio over SSH)
// ---------------------------------------------------------------------------

/// Streams raw `f32` PCM over a loopback TCP socket.
///
/// This is how you get audio out of a headless or SSH session: bind here, then
/// forward the port to your local machine and play the stream there.
///
/// ```text
/// # server
/// zboub69 --stream-port 9999
/// # local machine
/// ssh -N -L 9999:127.0.0.1:9999 user@server
/// ffplay -f f32le -ar 44100 -ch_layout stereo tcp://127.0.0.1:9999
/// ```
///
/// Nothing is rendered until a client connects, so the transport never runs
/// ahead of something nobody can hear.
pub struct TcpStreamBackend {
    sample_rate: u32,
    port: u16,
    listener: Option<TcpListener>,
    stop: Arc<AtomicBool>,
    active: Arc<Mutex<Option<TcpStream>>>,
    handle: Option<JoinHandle<()>>,
}

impl TcpStreamBackend {
    /// Bind the loopback stream port. Pass `0` to let the OS choose one.
    pub fn new(sample_rate: u32, port: u16) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|e| anyhow!("cannot bind 127.0.0.1:{port}: {e}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| anyhow!("cannot configure listener: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| anyhow!("cannot read stream address: {e}"))?
            .port();
        Ok(Self {
            sample_rate,
            port,
            listener: Some(listener),
            stop: Arc::new(AtomicBool::new(false)),
            active: Arc::new(Mutex::new(None)),
            handle: None,
        })
    }

    /// The port actually bound (useful when `0` was requested).
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl AudioBackend for TcpStreamBackend {
    fn name(&self) -> &'static str {
        "stream"
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn stream_port(&self) -> Option<u16> {
        Some(self.port)
    }

    fn start(&mut self, mut render: RenderFn) -> Result<()> {
        let listener = self
            .listener
            .take()
            .ok_or_else(|| anyhow!("stream listener already started"))?;
        let stop = self.stop.clone();
        let active = self.active.clone();
        let sample_rate = self.sample_rate;

        self.handle = Some(
            thread::Builder::new()
                .name("zboub-audio-stream".into())
                .spawn(move || {
                    let mut buffer = vec![0.0f32; CHUNK_FRAMES * 2];
                    let mut bytes = Vec::with_capacity(CHUNK_FRAMES * 2 * 4);
                    let period = chunk_period(sample_rate);

                    while !stop.load(Ordering::Relaxed) {
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let _ = stream.set_nodelay(true);
                                if let Ok(clone) = stream.try_clone()
                                    && let Ok(mut slot) = active.lock()
                                {
                                    *slot = Some(clone);
                                }

                                let mut deadline = Instant::now();
                                while !stop.load(Ordering::Relaxed) {
                                    render(&mut buffer);
                                    bytes.clear();
                                    for sample in &buffer {
                                        bytes.extend_from_slice(&sample.to_le_bytes());
                                    }
                                    // Blocks if the client is behind; paced in
                                    // software if the client is faster than us.
                                    if stream.write_all(&bytes).is_err() {
                                        break;
                                    }
                                    pace(&mut deadline, period);
                                }

                                if let Ok(mut slot) = active.lock() {
                                    *slot = None;
                                }
                            }
                            // No client yet: idle briefly and poll the stop flag.
                            Err(_) => thread::sleep(Duration::from_millis(5)),
                        }
                    }
                })
                .map_err(|e| anyhow!("failed to spawn stream thread: {e}"))?,
        );
        Ok(())
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Shutting the live socket down unblocks a writer parked in `write_all`.
        if let Ok(slot) = self.active.lock()
            && let Some(stream) = slot.as_ref()
        {
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// cpal backend
// ---------------------------------------------------------------------------

#[cfg(feature = "cpal")]
mod cpal_backend {
    use super::{AudioBackend, RenderFn};
    use anyhow::{Result, anyhow};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{SampleFormat, StreamConfig};

    /// Largest callback we will render into; larger callbacks are truncated.
    const MAX_CALLBACK_FRAMES: usize = 4096;

    /// Direct device output through cpal.
    pub struct CpalBackend {
        device: cpal::Device,
        config: StreamConfig,
        format: SampleFormat,
        sample_rate: u32,
        channels: usize,
        stream: Option<cpal::Stream>,
    }

    impl CpalBackend {
        /// Open the default output device.
        pub fn new() -> Result<Self> {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .ok_or_else(|| anyhow!("no default output device"))?;
            let supported = device
                .default_output_config()
                .map_err(|e| anyhow!("no output config: {e}"))?;
            let format = supported.sample_format();
            let config: StreamConfig = supported.config();
            Ok(Self {
                sample_rate: config.sample_rate.0,
                channels: config.channels as usize,
                device,
                config,
                format,
                stream: None,
            })
        }

        fn build<T>(&self, render: RenderFn) -> Result<cpal::Stream>
        where
            T: cpal::SizedSample + cpal::FromSample<f32>,
        {
            let channels = self.channels.max(1);
            let mut scratch = vec![0.0f32; MAX_CALLBACK_FRAMES * 2];
            let stream = self
                .device
                .build_output_stream(
                    &self.config,
                    move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
                        let frames = (out.len() / channels).min(MAX_CALLBACK_FRAMES);
                        let stereo = &mut scratch[..frames * 2];
                        render(stereo);
                        for frame in 0..frames {
                            let left = stereo[frame * 2];
                            let right = stereo[frame * 2 + 1];
                            for channel in 0..channels {
                                let value = match channel {
                                    0 => left,
                                    1 => right,
                                    _ => 0.0,
                                };
                                out[frame * channels + channel] = T::from_sample(value);
                            }
                        }
                    },
                    |err| eprintln!("[audio] stream error: {err}"),
                    None,
                )
                .map_err(|e| anyhow!("failed to build stream: {e}"))?;
            stream
                .play()
                .map_err(|e| anyhow!("failed to start stream: {e}"))?;
            Ok(stream)
        }
    }

    impl AudioBackend for CpalBackend {
        fn name(&self) -> &'static str {
            "cpal"
        }

        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn start(&mut self, render: RenderFn) -> Result<()> {
            let stream = match self.format {
                SampleFormat::F32 => self.build::<f32>(render)?,
                SampleFormat::I16 => self.build::<i16>(render)?,
                SampleFormat::U16 => self.build::<u16>(render)?,
                other => return Err(anyhow!("unsupported sample format {other:?}")),
            };
            self.stream = Some(stream);
            Ok(())
        }

        fn stop(&mut self) {
            if let Some(stream) = self.stream.take() {
                let _ = stream.pause();
            }
        }
    }

    /// Whether a device is available.
    pub fn available() -> bool {
        cpal::default_host().default_output_device().is_some()
    }
}

/// The chosen backend plus a human-readable note explaining the choice.
pub struct BackendChoice {
    pub backend: Box<dyn AudioBackend>,
    pub sample_rate: u32,
    pub note: String,
}

/// What the user asked for on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendPreference {
    /// Pick the best device available.
    Auto,
    /// Stream raw PCM over loopback TCP for `ssh -L` playback.
    Stream { port: u16 },
}

/// Pick the best backend available on this machine.
///
/// The note always explains the outcome so the UI can tell the user *why* it is
/// silent and what to install.
pub fn select_backend(preference: BackendPreference) -> BackendChoice {
    if let BackendPreference::Stream { port } = preference {
        return match TcpStreamBackend::new(DEFAULT_SAMPLE_RATE, port) {
            Ok(backend) => {
                let port = backend.port();
                BackendChoice {
                    backend: Box::new(backend),
                    sample_rate: DEFAULT_SAMPLE_RATE,
                    note: format!(
                        "streaming PCM on 127.0.0.1:{port} — connect a local player (see README: Audio over SSH)"
                    ),
                }
            }
            Err(error) => BackendChoice {
                backend: Box::new(NullBackend::new(DEFAULT_SAMPLE_RATE)),
                sample_rate: DEFAULT_SAMPLE_RATE,
                note: format!("silent: {error}"),
            },
        };
    }

    #[cfg(feature = "cpal")]
    {
        if cpal_backend::available() {
            if let Ok(backend) = cpal_backend::CpalBackend::new() {
                let sample_rate = backend.sample_rate();
                return BackendChoice {
                    backend: Box::new(backend),
                    sample_rate,
                    note: "direct device output".into(),
                };
            }
        }
    }

    match SubprocessBackend::new(DEFAULT_SAMPLE_RATE) {
        Ok(backend) => BackendChoice {
            backend: Box::new(backend),
            sample_rate: DEFAULT_SAMPLE_RATE,
            note: "playback through ffplay (install libasound2-dev and build with --features cpal for low latency)".into(),
        },
        Err(error) => BackendChoice {
            backend: Box::new(NullBackend::new(DEFAULT_SAMPLE_RATE)),
            sample_rate: DEFAULT_SAMPLE_RATE,
            note: format!("silent: {error}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn shared_state_round_trips() {
        let shared = SharedState::new(48_000);
        let mut peaks = [0.0f32; MAX_TRACKS];
        peaks[2] = 0.75;
        shared.publish(1_234, true, false, 5, &peaks);
        assert_eq!(shared.position_frames.load(Ordering::Relaxed), 1_234);
        assert!(shared.playing.load(Ordering::Relaxed));
        assert_eq!(shared.step_index.load(Ordering::Relaxed), 5);
        assert!((shared.peaks()[2] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn ffplay_never_receives_the_unknown_ac_flag() {
        let args = player_args(44_100);
        assert!(!args.iter().any(|a| a == "-ac"));
        assert!(args.windows(2).any(|w| w[0] == "-ch_layout" && w[1] == "stereo"));
    }

    #[test]
    fn null_backend_renders_and_stops() {
        let frames = Arc::new(AtomicUsize::new(0));
        let counter = frames.clone();
        let mut backend = NullBackend::new(44_100);
        let render: RenderFn = Box::new(move |out: &mut [f32]| {
            counter.fetch_add(out.len() / 2, Ordering::Relaxed);
        });
        backend.start(render).expect("start");
        thread::sleep(Duration::from_millis(80));
        backend.stop();
        assert!(frames.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn select_backend_always_returns_something_with_a_reason() {
        let choice = select_backend(BackendPreference::Auto);
        assert!(choice.sample_rate > 0);
        assert!(!choice.backend.name().is_empty());
        assert!(!choice.note.is_empty());
    }

    #[test]
    fn stream_backend_sends_audio_to_a_connected_client() {
        // Port 0 lets the OS pick, so parallel tests can never clash.
        let mut backend = TcpStreamBackend::new(44_100, 0).expect("bind stream port");
        let port = backend.port();
        let render: RenderFn = Box::new(|out: &mut [f32]| {
            for sample in out.iter_mut() {
                *sample = 0.5;
            }
        });
        backend.start(render).expect("start streaming");

        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        let mut chunk = [0u8; 64];
        let mut received = 0usize;
        while received < chunk.len() {
            match std::io::Read::read(&mut client, &mut chunk[received..]) {
                Ok(0) => break,
                Ok(n) => received += n,
                Err(_) => break,
            }
        }
        backend.stop();

        assert!(received > 0, "client should receive PCM bytes");
        // 0.5f32 little-endian is 00 00 00 3F.
        assert_eq!(&chunk[0..4], &0.5f32.to_le_bytes());
    }
}
