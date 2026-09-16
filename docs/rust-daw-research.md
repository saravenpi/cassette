# Rust DAW Research — Architectural Patterns from Real Codebases

Research compiled 2026-09-16 from source code, documentation, and blog posts of
working Rust DAW projects. All code links point to live repositories.

---

## 1. Real-World Rust DAW Projects

### 1.1 `naiqvist/daw` — Custom engine, egui/wgpu UI
- **Repo:** https://github.com/naiqvist/daw
- **Status:** Active, 296+ commits, 3 contributors (human + AI-assisted)
- **Architecture:** Custom audio engine on rtaudio/JACK duplex, egui/wgpu UI
- **Scale:** 60+ device types (kick, snare, hat, synth, EQ, filter, limiter, sampler, etc.), DSP kernels, sequencer, piano roll, mixer, transport, bouncing/rendering
- **Key files:** `src/audio/graph.rs`, `src/audio/transport.rs`, `src/audio/engine.rs`, `src/dsp/` (mem, arith, ramps, reverb)
- **Notable:** Strict `AGENTS.md` red-zone contract: audio callback never allocates, locks, syscalls, logs, panics, or runs unbounded-time paths. Uses `rtrb` ring buffer for UI→audio commands, `triple_buffer` for audio→UI state. DSP kernels run under `assert_no_alloc` in debug builds.
- **Backend:** rtaudio 0.8.0 with `jack_linux` feature (not cpal)
- **Plugin host:** CLAP via clack-host

### 1.2 `jcosta33/sourdaw` — Unified Rust DSP, Electron shell
- **Repo:** https://github.com/jcosta33/sourdaw
- **Status:** Active, large codebase (~12,500 files), 144k stars on the HN post
- **Architecture:** Rust DSP engine (`daw-engine` + `daw-dsp` crates) with an Electron desktop shell and Web Audio integration for plugins
- **Key crates:**
  - `daw-engine` — Audio thread, scheduler, timeline, MIDI, plugins, transport, offline rendering
  - `daw-dsp` — DSP algorithms (grand_boule physical modeling strings, bacteria effects, filters, dynamics, etc.)
- **Notable:** Extremely sophisticated audio thread (`audio_thread.rs`) with render stall detection, retirement/reclamation for plugin destructors, multiple diagnostic channels (graph progress, transport position, meters, engine events), atomic state publishing
- **Key insight:** `MAX_CALLBACK_FRAMES = 4096`, `PREFERRED_BUFFER_FRAMES = 512` (professional default matching Live/Logic/Reaper)

### 1.3 `generic-daw` / MeadowlarkDAW — Modular engine + GUI
- **Repo:** https://github.com/generic-daw/generic-daw
- **Status:** Active, multiple workspace crates
- **Architecture:** `audio_graph` crate provides DAG-based parallel audio processing with thread pool
- **Key crates:** `audio_graph`, `dsp`, `clap_host`, `generic_daw_core`, `generic_daw_gui`, `generic_daw_project`, `generic_daw_widget`, `thread_pool`, `utils`
- **Notable:** `AudioGraph<Node>` uses `crossbeam_queue::ArrayQueue` for work scheduling, Kahn's algorithm for DAG ordering, lock-free routing between nodes (no allocations per block)
- **Related:** MeadowlarkDAW/meadowlark (full DAW, moved to Codeberg) uses this as backend

### 1.4 `whoisryosuke/daw-tauri` — Tauri + cpal teaching project
- **Repo:** https://github.com/whoisryosuke/daw-tauri
- **Blog series:** https://whoisryosuke.com/blog/2026/creating-a-daw-in-rust/
- **Architecture:** Tauri (Rust backend + React frontend), cpal for audio, ringbuf for command passing
- **Key patterns:** Mixer with AudioNode enum (Silence/StaticBuffer/Streaming/Synthesizer), ringbuf command channel (producer→consumer, lock-free), waveform visualization via reverse ringbuf + separate thread, playback time via AtomicU64

### 1.5 Community ecosystem
- **Rust Audio:** https://rust.audio/ — Community hub, Discord, crate listings
- **RustAudio GitHub:** https://github.com/RustAudio — cpal, dasp, rust-jack, dsp-chain, vst-rs, nih-plug, etc.
- **Meadowlark:** https://github.com/MeadowlarkDAW/Meadowlark — Full-featured DAW (moved to Codeberg, on hiatus since 2023 but architecturally instructive)

---

## 2. Event Systems and Multi-Component Communication

### 2.1 The canonical pattern: ring buffer + command enum

From `whoisryosuke/daw-tauri` (the most documented pattern):

```rust
// Lock-free command channel from UI thread → audio thread
let heap = HeapRb::<AudioCommand>::new(128);
let (producer, mut consumer) = heap.split();

// Audio thread owns the consumer; UI thread owns the producer
pub struct AudioEngine {
    stream: cpal::Stream,
    producer: HeapProd<AudioCommand>,
    asset_store: AssetStore,
}

enum AudioCommand {
    Play(Vec<f32>),
    Pause,
}
```

**Why this pattern:** The audio callback runs in a closure that `move`s ownership of the mixer in.
Without a ring buffer, you'd need `Arc<Mutex<Mixer>>` which means locking inside the audio thread — forbidden for real-time audio.

### 2.2 SPSC vs MPSC ring buffers

The same project documents the evolution:

1. **SPSC (`ringbuf` `HeapRb`):** Natural fit for single-producer (UI) → single-consumer (audio).
Lock-free. But `HeapCons` doesn't implement `Send`, so you can't share the producer across multiple UI threads.
2. **MPSC (`crossbeam::channel::bounded`):** When you need multiple producers (multiple Tauri handlers), swap to crossbeam.
Same API shape (`try_send`/`try_recv`), slightly higher overhead but supports multiple senders.

```rust
// SPSC — when one producer is enough
let (producer, consumer) = crossbeam::channel::bounded::<AudioCommand>(128);
```

### 2.3 Typed events on bounded queues

From `TerminalDAW` (`src/events/bus.rs`):

```rust
pub struct EventBus {
    tx: Arc<Sender<Event>>,
    subscribers: Vec<Arc<Sender<Event>>>,
}

#[derive(Debug, Clone)]
pub enum Event {
    KeyPress(String),
    Quit,
    Play,
    Stop,
    Record,
    TransportTick,
    NoteOn(u8, u8, u8),
    NoteOff(u8, u8, u8),
}
```

**Key design decision:** Events carry *intent* (Play, NoteOn), not *data* (raw bytes, envelopes).
The bus is bounded — excess events are dropped rather than buffered indefinitely.
This keeps tail latency predictable.

### 2.4 Naqvist's approach: flat compiled schedule

`daw` uses a different philosophy — a **flat compiled schedule** over a 64-byte-aligned arena (see `src/audio/graph.rs`):

- Nodes are compiled into a scheduled sequence at edit time
- Audio callback iterates the schedule linearly (cache-friendly, no graph traversal at runtime)
- Stereo slot-pairs managed via ring buffer (`rtrb`)
- Box swaps defer drops off-thread (audio thread never deallocates)

This trades dynamic graph flexibility for real-time safety: editing recompiles the schedule; playback runs the pre-compiled plan.

### 2.5 generic-daw's DAG execution

`audio_graph` processes nodes in topological order with a thread pool:

```rust
impl<Node: NodeImpl> WorkList for AudioGraph<Node> {
    fn next_item(&self) -> Option<Self::Item> { self.queue.pop() }
    fn do_work(&self, item: Self::Item, ...) -> Option<Self::Item> {
        self.process_node(&self.graph[&item], ...)
    }
}
```

- Kahn's algorithm for ordering; `indegree` atomic counters track readiness
- Each `Entry` has `node: Mutex<Node>` + `buffers: RwLock<Buffers<Node>>` — node state and audio buffers are separately locked
- Process function signature: `fn process(&mut self, state: &State, audio: &mut [[f32; 2]], events: &mut Vec<Event>, scratch: &mut Scratch, injector: &Injector) -> usize` (returns latency)

---

## 3. Real-Time Audio Performance Patterns

### 3.1 The audio callback red-zone rules

Every mature Rust DAW enforces these rules, often documented as a contract:

| Rule | Rationale |
|---|---|
| No heap allocation after startup | `Box::new()`, `Vec::push()` can block on the global allocator |
| No locks (Mutex, RwLock) in callback | Lock contention → missed deadlines → glitches/xruns |
| No I/O (logging, files, network) | Syscalls are unbounded in duration |
| No panics/unwrap/expect | Panic in audio thread = crash/dropout |
| No unbounded loops | Must complete within the block period |
| Pre-allocate all buffers | Buffers allocated once at startup |

From `daw/AGENTS.md`:
> The audio callback (red zone) never: allocates, takes locks, makes syscalls, logs, panics/unwrap/expect, or runs unbounded-time paths.

### 3.2 Thread architecture

Standard thread topology (from `sourdaw/audio_thread.rs`):

```
┌─────────────────────────────────────────────────┐
│ Control Thread (UI, Tauri, egui)                │
│  └─ sends commands via ring buffer (SPSC/MPSC)  │
│  └─ receives state via triple_buffer/atomics    │
├─────────────────────────────────────────────────┤
│ Audio Owner Thread (sponsored by cpal)          │
│  └─ runs device open/close, capture opens       │
│  └─ spawns Render Stall Watch thread            │
│  └─ spawns Plugin Reclaimer thread              │
├─────────────────────────────────────────────────┤
│ Audio Render Callback (cpal callback)           │
│  └─ processes audio graph                       │
│  └─ writes to device output buffer              │
│  └─ ONLY: ring buffer pop, atomic ops,          │
│     scratch buffer reads, node.process()        │
├─────────────────────────────────────────────────┤
│ Waveform/Viz Thread (optional)                  │
│  └─ drains reverse ring buffer from audio       │
│  └─ batches and emits to UI at ~60fps           │
└─────────────────────────────────────────────────┘
```

### 3.3 Lock-free state passing

From `sourdaw` — multiple SPSC ring buffers for different data types:

```rust
// Commands from UI → audio
let (command_tx, command_rx): RingBuffer<GraphCommand> = ...

// Audio events from audio → UI
let (event_tx, event_rx): RingBuffer<EngineEvent> = ...

// Transport position (atomic, sampled per callback)
transport_position: AtomicU64

// Meter snapshots
meter_tx: Input<MeterSnapshot>

// Graph progress (for UI loading indicator)
graph_progress_tx: Input<GraphProgressSnapshot>
```

From `whoisryosuke/daw-tauri` — playback time via atomic:

```rust
let playback_time = Arc::new(AtomicU64::new(0));
// Audio callback: playback_time.fetch_add(1, Ordering::SeqCst) per frame
// UI thread: playback_time.load(Ordering::SeqCst) every 16ms
```

### 3.4 Pre-allocation patterns

From `whoisryosuke/daw-tauri` — avoiding allocations in the audio stream:

```rust
const MAX_NODES: usize = 32;

struct Mixer {
    nodes: [Option<AudioNodeTypes>; MAX_NODES],
}

impl Mixer {
    fn new() -> Self {
        Self { nodes: [const { None }; MAX_NODES] }
    }

    fn add_node(&mut self, node: AudioNodeTypes) {
        for slot in self.nodes.iter_mut() {
            if slot.is_none() {
                *slot = Some(node);
                return;
            }
        }
    }
}
```

Key insight: `Option` pre-allocates the memory for the node without constructing it.
The audio thread pays zero allocation cost when adding a node — it just writes into a pre-reserved slot.

### 3.5 Sample-accurate timing

From `sourdaw/audio_thread.rs`:
- `PREFERRED_BUFFER_FRAMES = 512` (professional default matching Live/Logic/Reaper)
- `MAX_CALLBACK_FRAMES = 4096` (safety ceiling; ~93ms at 44.1kHz)
- `negotiated_buffer_size()` gently negotiates with device preferences — only intervenes when the device range allows ≤4096 frames
- Playback position tracked as sample count (not wall-clock time) in the audio callback, published via atomics
- Transport position is converted from seconds → frames on *the audio thread's clock* (sample rate decided there)

From `daw` — dedicated `Transport` module with `TimeMap` as the sole time conversion authority, segment splitter, sample-accurate loop points, discontinuity handling.

### 3.6 Defensive audio thread safety

From `sourdaw` — panic catching at audio thread boundaries:

```rust
fn reclaim_retired<T>(retired: T) {
    if catch_unwind(AssertUnwindSafe(|| drop(retired))).is_err() {
        eprintln!("[Engine] Plugin destructor panicked during retirement");
    }
}
```

And in `sourdaw`'s owner thread:
```rust
OwnerCommand::OpenCapture => {
    if catch_unwind(AssertUnwindSafe(|| on_capture(stream))).is_err() {
        eprintln!("[Engine] Audio capture open panicked");
    }
}
```

**Pattern:** Destructors and device code running on audio-adjacent threads are wrapped in `catch_unwind` so a panic cannot take down the audio stream.

---

## 4. Clean Architecture vs Pragmatic Implementations

### 4.1 Clean architecture: TerminalDAW (documented skeleton)

`docs/ARCHITECTURE.md` defines strict layer rules:

```
input ──► events ──► core ◄── ui
                      ▲
                    models (shared data only)
```

| Layer | Allowed deps | Forbidden |
|---|---|---|
| `core` | `events::types`, `models`, `utils` | `ui`, `input` |
| `ui` | `core`, `events`, `models`, `utils` | `input` |
| `input` | `events`, `core` (read-only types) | `ui` |
| `utils` | nothing external | — |
| `models` | nothing external | — |

**Principles:** UI/core independence, input as adapter, small typed events, thin TUI, bounded queues everywhere, real-time audio safety.

### 4.2 Pragmatic architecture: whoisryosuke's Tauri DAW

The Tauri DAW uses a layered but pragmatic structure:

```
AudioEngine {
    stream: cpal::Stream,          // the audio stream (lives forever)
    producer: HeapProd<AudioCommand>, // ring buffer producer
    asset_store: AssetStore,        // cached samples (Mutex<HashMap>)
}

Mixer {
    nodes: Vec<AudioNodeTypes>,     // enum of node types (no Box needed)
}

// Mixer is moved INTO the audio callback closure (not behind Arc<Mutex>)
// This gives it unfettered access — no locking needed.
```

**Key pragmatic decisions:**
- `AssetStore` behind `Mutex` — OK because it's on the UI/control thread, never accessed from audio
- `Mixer` moved into the closure — no synchronization needed at all
- `Arc<AudioBuffer>` for zero-cost sample sharing between AssetStore and AudioNode
- `AudioNode` as **enum** (not trait object) — avoids `Box<dyn>` allocation overhead; all variants get `get_sample()` via impl

### 4.3 naiqvist's contract-driven architecture

`daw/AGENTS.md` enforces contracts via code (not just documentation):

```
Audio modules get #![deny(clippy::unwrap_used, clippy::expect_used)]
and run under assert_no_alloc in debug builds.
```

**Red zone vs green zone:**
- **Red zone (audio callback):** `rtrb` ring buffer, `triple_buffer`, `basedrop` for deferred drops. Nothing else.
- **Green zone (UI/control):** `parking_lot`, `crossbeam-channel`, serde, anything heap-hungry

**Compile-time enforcement:** `#[cfg(test)]` runs `assert_no_alloc` on all DSP kernels.
Each kernel must pass 5 mandatory tests: reference, split-block equivalence, no-alloc, edge lengths, denormal tail.

### 4.4 The mixer pattern: trait vs enum vs closure

All three approaches appear in the ecosystem:

**Trait object** (flexible but boxed):
```rust
pub trait AudioNode { fn get_sample(&mut self) -> Option<f32>; }
nodes: Vec<Box<dyn AudioNode>>
```

**Enum** (zero-cost, known variants):
```rust
pub enum AudioNodeTypes {
    Silence,
    StaticBuffer(SampleNode),
    Streaming(SampleNode),
    Synthesizer(SynthNode),
}
impl AudioNodeTypes {
    fn get_sample(&mut self) -> Option<f32> { match self { ... } }
}
nodes: Vec<AudioNodeTypes>
```

**Closure** (simplest, single purpose):
```rust
let signal = move || { /* oscillator */ };
cpal stream: |output| { for frame in output { /* call signal */ } }
```

---

## 5. Module Organization and Separation of Concerns

### 5.1 Typical crate structure (multi-crate workspace)

From `generic-daw/Cargo.toml`:

```toml
[workspace]
members = [
    "audio_graph", "clap_host", "dsp",
    "generic_daw_core", "generic_daw_gui",
    "generic_daw_project", "generic_daw_widget",
    "thread_pool", "utils",
]
```

**Separation:** Audio graph is its own crate — UI, plugins, and project management all depend on it, but it has zero dependency on them.

### 5.2 Single-crate module structure

From `naiqvist/daw` (single crate, ~100+ modules):

```
src/
├── audio/         # Engine, graph, transport, all nodes, DSP
├── ui/            # egui panels, device cards, theming, palette
├── dsp/           # DSP kernels (mem, arith, ramps, reverb, filters, etc.)
├── engine.rs      # Top-level orchestration
├── lib.rs         # Crate root
├── main.rs        # Binary entry
└── ...
```

### 5.3 TerminalDAW module structure (current project)

```
src/
├── main.rs        # Entry point
├── app.rs         # Orchestrator
├── core/          # Audio engine, MIDI, tracks, clips, mixer (no UI dep)
│   ├── engine.rs
│   ├── audio.rs   # AudioBuffer (real-time safe surface)
│   ├── midi.rs
│   ├── track.rs
│   ├── clip.rs
│   └── mixer.rs
├── ui/            # Rendering, layout, views
├── events/        # EventBus, typed events
├── input/         # Keyboard mapping, input handling
├── models/        # Domain data (no behaviour)
└── utils/         # Pure helpers (math, time, frequency)
```

**Dependency direction (from `docs/ARCHITECTURE.md`):**
```
input ──► events ──► core ◄── ui
                          ▲
                        models
```

### 5.4 Audio thread vs control thread boundary

The most important architectural decision in any Rust DAW: **the audio callback closure must own everything it touches**.
From `whoisryosuke/daw-tauri`:

> Because we used `move` in the cpal callback, it takes ownership of any variable we use inside. This means when we run the mixer inside, it moves from the new() method to inside our audio stream...forever.

**Practical implication:** Data structures accessed from the audio thread must be designed to be moved into the closure once.
Shared state uses ring buffers (not locks). The `Mixer` lives inside the stream, giving it direct access with zero synchronization overhead.

### 5.5 Event flow patterns

Three distinct event flows in a mature Rust DAW:

| Flow | Direction | Mechanism | Real-time safe? |
|---|---|---|---|
| Commands (UI→audio) | Control thread → audio thread | SPSC ring buffer | Yes (lock-free pop) |
| State (audio→UI) | Audio thread → control thread | Triple buffer or atomic | Yes (lock-free read) |
| Events (UI↔UI) | Between UI panels | mpsc or event bus | N/A (not audio path) |

---

## 6. Key Libraries and Crates

| Crate | Role | Notes |
|---|---|---|
| `cpal` | Low-level audio I/O | Cross-platform; audio stream callbacks |
| `rtaudio` | Audio I/O (alternative) | Better JACK support on Linux; `rtrb`-compatible |
| `rtrb` | SPSC ring buffer | The go-to for audio thread communication; lock-free |
| `triple_buffer` | Triple-buffered state | Audio→UI state without locks |
| `crossbeam-channel` | MPSC channels | Multiple producers; bounded; `try_send`/`try_recv` |
| `ringbuf` | SPSC ring buffer | `HeapRb` for cross-thread, `StaticRb` for single-thread |
| `dasp` | DSP primitives | Signal processing, sample conversion, synth examples |
| `dsp-chain` | Audio DSP chaining | Plugin/effect chain processing |
| `audio_graph` | DAG audio processing | Parallel graph execution, thread pool |
| `auxide` | Audio graph kernel | RT-safe deterministic graph execution |
| `nih-plug` | Plugin host | VST3/LV2 host; from Rust Audio ecosystem |
| `clack-host` | CLAP host | Native plugin hosting (used by `daw`) |
| `egui` | Immediate-mode UI | Used by `daw`; fast but redraws every frame |
| `ratatui` | Terminal UI | Used by TerminalDAW; retained-mode via terminal |
| `cpal` + `symphonia` | Audio I/O + decoding | File loading in Tauri DAW |

---

## 7. Key Takeaways for TerminalDAW

Based on this research, the current TerminalDAW skeleton (`src/`) should evolve:

1. **Replace `std::sync::mpsc` EventBus with `rtrb` or `crossbeam::bounded`** — the current unbounded channel risks memory growth under load.
`ARCHITECTURE.md` already says "bounded" but `bus.rs` uses `channel()` (unbounded).

2. **Move Mixer/AudioBuffer design to be pre-allocated** — follow the `[Option<AudioNode>; 32]` pattern.
No `Vec::push()` in any audio path.

3. **Define the audio thread boundary explicitly** — which modules run on the audio callback vs control thread.
The `AGENTS.md` pattern from `daw` (red zone/green zone) is directly applicable.

4. **Keep `core/` free of UI dependency** — this is already correct per the architecture doc.
Any future cpal/rtaudio integration should go behind a trait in `core/audio.rs` so it can be swapped without touching UI.

5. **Events should be small and typed** — already done in `events/types.rs`. Avoid sending raw sample data over the bus; send commands that reference pre-allocated buffers.

---

*Sources: All code linked above is live and accessible.
Blog post at https://whoisryosuke.com/blog/2026/creating-a-daw-in-rust/ contains full source for the Tauri DAW pattern at https://github.com/whoisryosuke/daw-tauri.*
