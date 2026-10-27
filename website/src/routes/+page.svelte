<script lang="ts">
  const features = [
    {
      title: "ZERO ALLOCATION CALLBACK",
      summary: "Lock-free real-time audio thread",
      description: "The audio rendering loop runs on a dedicated high-priority thread with pre-allocated memory buffers. No heap allocations, no locks, and zero garbage collection spikes during live performance."
    },
    {
      title: "STEREO INSERT EFFECTS",
      summary: "Modular DSP chain per channel",
      description: "Each track hosts a custom insert chain featuring chorus, tape delay, plate reverb, overdrive distortion, bitcrusher, resonant state-variable filter, compressor with sidechain, limiter, and 3-band EQ."
    },
    {
      title: "PIANO ROLL EDITOR",
      summary: "Precision MIDI note sequencing",
      description: "Inspect and edit note patterns with velocity bars, configurable grid snapping from whole notes down to 1/64th triplets, box selection, duplicate, quantize, and scale folding."
    },
    {
      title: "HIGH-DPI TIMELINE",
      summary: "Sample accurate audio and MIDI blocks",
      description: "Session timeline with sample waveform generation, block splitting, length trimming, loop markers, smooth horizontal zoom, and instant audio sample preview."
    },
    {
      title: "PRESET SYNTHESIZER",
      summary: "Polyphonic subtractive and wavetable engine",
      description: "Built-in synth with dual anti-aliased oscillators, envelope generators, low-pass filter with resonance, LFO routing, and immediate playability via QWERTY keyboard."
    },
    {
      title: "NATIVE COMPILATION",
      summary: "Lightweight footprint, instant launch",
      description: "Compiled directly to machine code via Rust and packaged into a standalone native binary with a webview surface. Starts in under 200ms with negligible CPU idle overhead."
    }
  ];

  const specs = [
    { label: "AUDIO ENGINE", value: "Pure Rust (cpal, ringbuf)" },
    { label: "FRONTEND STACK", value: "SvelteKit 5 + TypeScript + Vite" },
    { label: "DESKTOP WRAPPER", value: "Tauri v2 shell" },
    { label: "CALLBACK ALLOCATION", value: "0 bytes on audio thread" },
    { label: "DSP RESOLUTION", value: "32-bit floating point stereo" },
    { label: "SUPPORTED PLATFORMS", value: "Windows 10/11, macOS 12+, Linux (X11/Wayland)" }
  ];

  const effectsList = [
    "CHORUS", "DELAY", "REVERB", "DISTORTION", "BITCRUSHER", "FILTER", "COMPRESSOR", "LIMITER", "PARAMETRIC EQ"
  ];
</script>

<svelte:head>
  <title>Cassette — Modern Session DAW with Rust Audio Engine</title>
  <meta
    name="description"
    content="Cassette is an open-source session DAW inspired by Ableton Live, featuring a zero-allocation Rust audio engine and a modern SvelteKit frontend."
  />
</svelte:head>

<div class="max-w-6xl mx-auto px-6 pt-16 pb-24 space-y-24">
  <section class="space-y-6 pt-4">
    <div class="text-xs text-[#86A881] uppercase tracking-widest">
      OPEN SOURCE DIGITAL AUDIO WORKSTATION
    </div>

    <h1 class="text-4xl sm:text-6xl uppercase tracking-tight leading-none max-w-4xl">
      A SESSION DAW WITH A RUST AUDIO ENGINE AND A SVELTEKIT FRONTEND
    </h1>

    <p class="text-base sm:text-lg text-[#FFFFFF]/70 max-w-2xl leading-relaxed">
      Cassette combines the low-latency precision of a hard real-time Rust audio
      engine with the fluidity of a modern reactive interface. Lay out MIDI and audio
      tracks on a beat timeline, craft sound with built-in DSP effects, and perform live.
    </p>

    <div class="flex flex-wrap items-center gap-4 pt-4">
      <a
        href="/download"
        class="inline-flex items-center gap-3 bg-[#86A881] text-[#0B0B0B] px-6 py-3.5 text-xs uppercase tracking-widest hover:bg-[#FFFFFF] transition-colors"
      >
        <span>&gt; DOWNLOAD CASSETTE</span>
      </a>

      <a
        href="https://github.com/saravenpi/cassette"
        target="_blank"
        rel="noreferrer"
        class="inline-flex items-center gap-3 border border-[#333333] text-[#FFFFFF] px-6 py-3.5 text-xs uppercase tracking-widest hover:border-[#86A881] hover:text-[#86A881] transition-colors"
      >
        <span>GITHUB REPOSITORY ↗</span>
      </a>
    </div>
  </section>

  <section class="border border-[#262626] bg-[#1F1F1F] p-4 sm:p-6 space-y-4">
    <div class="flex items-center justify-between border-b border-[#262626] pb-3 text-xs text-[#FFFFFF]/60">
      <div class="flex items-center gap-3">
        <span class="inline-block w-2.5 h-2.5 rounded-full bg-[#86A881]"></span>
        <span class="text-[#86A881] tracking-widest uppercase">LIVE SESSION VIEW</span>
        <span class="text-[#262626]">|</span>
        <span class="font-mono text-[11px]">120.0 BPM · 4/4 · 44.1 kHz</span>
      </div>
      <div class="text-[11px] uppercase tracking-widest text-[#FFFFFF]/40 hidden sm:block">
        ENGINE: RT SOUNDING
      </div>
    </div>

    <div class="bg-[#0B0B0B] border border-[#262626] p-4 sm:p-6 font-mono text-xs overflow-x-auto space-y-4">
      <div class="grid grid-cols-12 gap-3 text-[10px] text-[#FFFFFF]/40 uppercase tracking-widest border-b border-[#1F1F1F] pb-2">
        <div class="col-span-3">TRACK</div>
        <div class="col-span-2">TYPE</div>
        <div class="col-span-4">ACTIVE DEVICES</div>
        <div class="col-span-3 text-right">METERS</div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center border-b border-[#1F1F1F]/60 pb-3">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#86A881]"></span>
          <span>01 SUB BASS</span>
        </div>
        <div class="col-span-2 text-[#86A881]">SYNTH / MIDI</div>
        <div class="col-span-4 text-[#FFFFFF]/70 text-[11px]">PolySynth &gt; Filter &gt; Distortion</div>
        <div class="col-span-3 text-right text-[#86A881] text-[11px] font-mono">
          [||||||||||||....] -4.2 dB
        </div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center border-b border-[#1F1F1F]/60 pb-3">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#8FB8D8]"></span>
          <span>02 DRUM STEM</span>
        </div>
        <div class="col-span-2 text-[#8FB8D8]">AUDIO WAVE</div>
        <div class="col-span-4 text-[#FFFFFF]/70 text-[11px]">Compressor &gt; Parametric EQ &gt; Limiter</div>
        <div class="col-span-3 text-right text-[#86A881] text-[11px] font-mono">
          [||||||||||||||..] -1.8 dB
        </div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center border-b border-[#1F1F1F]/60 pb-3">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#D9B26F]"></span>
          <span>03 LEAD PLUCK</span>
        </div>
        <div class="col-span-2 text-[#D9B26F]">SYNTH / MIDI</div>
        <div class="col-span-4 text-[#FFFFFF]/70 text-[11px]">PolySynth &gt; Chorus &gt; Tape Delay &gt; Reverb</div>
        <div class="col-span-3 text-right text-[#86A881] text-[11px] font-mono">
          [||||||||........] -8.4 dB
        </div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#C87D55]"></span>
          <span>04 MASTER</span>
        </div>
        <div class="col-span-2 text-[#C87D55]">BUS</div>
        <div class="col-span-4 text-[#FFFFFF]/70 text-[11px]">Bus Compressor &gt; Master Limiter</div>
        <div class="col-span-3 text-right text-[#86A881] text-[11px] font-mono">
          [|||||||||||||||.] -0.3 dB
        </div>
      </div>
    </div>
  </section>

  <section id="features" class="space-y-12">
    <div class="flex flex-col md:flex-row md:items-end justify-between gap-4 border-b border-[#262626] pb-6">
      <div class="space-y-2">
        <div class="text-xs text-[#86A881] uppercase tracking-widest">
          CORE CAPABILITIES
        </div>
        <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">ENGINEERED FOR STABILITY AND FLOW</h2>
      </div>
      <div class="text-xs text-[#FFFFFF]/50 uppercase tracking-widest">
        ZERO UNNECESSARY ABSTRACTIONS
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      {#each features as feature}
        <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-4 hover:border-[#86A881]/50 transition-colors">
          <div class="space-y-1">
            <h3 class="text-sm uppercase tracking-wider text-[#FFFFFF]">{feature.title}</h3>
            <div class="text-xs text-[#86A881] tracking-wide">{feature.summary}</div>
          </div>
          <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
            {feature.description}
          </p>
        </div>
      {/each}
    </div>
  </section>

  <section class="space-y-8 border-t border-[#262626] pt-16">
    <div class="space-y-2">
      <div class="text-xs text-[#86A881] uppercase tracking-widest">
        BUILT-IN INSERT EFFECTS
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">ZERO-LATENCY STEREO DSP SUITE</h2>
    </div>

    <p class="text-xs sm:text-sm text-[#FFFFFF]/70 max-w-2xl leading-relaxed">
      Every effect is crafted in Rust with clean mathematical models, avoiding third-party
      bloat and allocation bugs. Reorder them on the fly, modulate parameters in real time,
      and route sidechain compression between any tracks.
    </p>

    <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-3 gap-3 pt-2">
      {#each effectsList as effect}
        <div class="border border-[#262626] bg-[#1F1F1F] p-4 text-center">
          <span class="text-xs uppercase tracking-widest text-[#FFFFFF]">{effect}</span>
        </div>
      {/each}
    </div>
  </section>

  <section id="architecture" class="space-y-8 border-t border-[#262626] pt-16">
    <div class="space-y-2">
      <div class="text-xs text-[#86A881] uppercase tracking-widest">
        ARCHITECTURE
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">THE SPLIT PIPELINE DESIGN</h2>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
      <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-3">
        <div class="text-xs text-[#86A881] uppercase tracking-widest">AUDIO ENGINE (RUST)</div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          Owns audio stream callbacks, MIDI decoding, synthesis voices, and effect calculations.
          Communicates with the app strictly through lock-free ring buffers without mutexes.
        </p>
      </div>

      <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-3">
        <div class="text-xs text-[#86A881] uppercase tracking-widest">DESKTOP IPC (TAURI)</div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          Serializes state snapshots at screen refresh rates and delivers asynchronous intent
          commands (block creation, notes, track reordering) to the engine thread safely.
        </p>
      </div>

      <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-3">
        <div class="text-xs text-[#86A881] uppercase tracking-widest">INTERFACE (SVELTEKIT)</div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          Draws waveforms, timeline markers, and interactive knobs using standard web primitives.
          A crash in the webview never interrupts audio playback or drops sounding voices.
        </p>
      </div>
    </div>
  </section>

  <section id="specs" class="space-y-8 border-t border-[#262626] pt-16">
    <div class="space-y-2">
      <div class="text-xs text-[#86A881] uppercase tracking-widest">
        TECHNICAL SPECIFICATIONS
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">ENGINE AND DESKTOP TARGETS</h2>
    </div>

    <div class="border border-[#262626] divide-y divide-[#262626] bg-[#1F1F1F]">
      {#each specs as spec}
        <div class="p-4 sm:p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-2 text-xs">
          <span class="text-[#FFFFFF]/50 uppercase tracking-widest">{spec.label}</span>
          <span class="text-[#FFFFFF] font-mono text-[11px] sm:text-xs">{spec.value}</span>
        </div>
      {/each}
    </div>
  </section>

  <section class="border border-[#86A881] bg-[#1F1F1F] p-8 sm:p-12 space-y-6 text-center">
    <div class="text-xs text-[#86A881] uppercase tracking-widest">READY TO PRODUCE</div>
    <h2 class="text-3xl sm:text-4xl uppercase tracking-tight">GET STARTED WITH CASSETTE TODAY</h2>
    <p class="text-xs sm:text-sm text-[#FFFFFF]/70 max-w-xl mx-auto leading-relaxed">
      Download the latest release for your platform or clone the repository to build from source.
      Free, open source, and lightweight.
    </p>
    <div>
      <a
        href="/download"
        class="inline-flex items-center gap-3 bg-[#86A881] text-[#0B0B0B] px-8 py-4 text-xs uppercase tracking-widest hover:bg-[#FFFFFF] transition-colors"
      >
        <span>&gt; GO TO DOWNLOADS</span>
      </a>
    </div>
  </section>
</div>
