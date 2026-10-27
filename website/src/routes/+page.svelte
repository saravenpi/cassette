<script lang="ts">
  const features = [
    {
      title: "Glitch-free playback",
      summary: "Zero-allocation audio loop",
      description: "The audio thread never allocates memory or locks during playback. Reorder tracks, tweak reverbs, or add clips in the middle of a set without a single dropout."
    },
    {
      title: "Modular insert chain",
      summary: "Nine stereo effects per channel",
      description: "Each track runs its own effects chain. Chorus, tape delay, plate reverb, overdrive, bitcrusher, state-variable filter, compressor with sidechain, limiter, and 3-band EQ."
    },
    {
      title: "Fast piano roll",
      summary: "Direct note and velocity editing",
      description: "Draw and duplicate notes with velocity bars. Grid snapping scales from whole notes down to 1/64 triplets, with quantize and scale folding built right in."
    },
    {
      title: "Beat timeline",
      summary: "Sample-accurate audio and MIDI blocks",
      description: "Slice, duplicate, loop, and drag audio samples and MIDI clips. Waveforms render instantly on high-refresh displays."
    },
    {
      title: "Built-in polysynth",
      summary: "Subtractive sound engine",
      description: "Dual anti-aliased oscillators, resonant low-pass filter, ADSR envelopes, and LFO modulation. Play it with your MIDI controller or computer keyboard immediately."
    },
    {
      title: "Instant startup",
      summary: "Lightweight and fully offline",
      description: "Starts in under 200 milliseconds. Takes less than 20 megabytes on disk. No login, no telemetry, and no background services."
    }
  ];

  const specs = [
    { label: "Audio processing", value: "32-bit floating point stereo" },
    { label: "Audio buffer", value: "Lock-free real-time callback, zero allocations" },
    { label: "Sample rates", value: "44.1 kHz, 48 kHz, 96 kHz, 192 kHz" },
    { label: "Footprint", value: "Single binary under 20 MB" },
    { label: "License", value: "Open source, MIT" },
    { label: "Supported operating systems", value: "Windows 10/11, macOS (Apple Silicon and Intel), Linux" }
  ];

  const effectsList = [
    "Chorus", "Tape delay", "Plate reverb", "Overdrive", "Bitcrusher", "State filter", "Compressor", "Limiter", "Parametric EQ"
  ];
</script>

<svelte:head>
  <title>Cassette — Session Audio Workstation</title>
  <meta
    name="description"
    content="Cassette is a fast, lightweight session DAW for recording, beatmaking, and live performance with zero-latency audio and built-in synth."
  />
</svelte:head>

<div class="max-w-6xl mx-auto px-6 pt-16 pb-24 space-y-24">
  <section class="space-y-6 pt-4">
    <div class="text-xs text-[#86A881] uppercase tracking-widest">
      Open source audio workstation
    </div>

    <h1 class="text-4xl sm:text-6xl uppercase tracking-tight leading-none max-w-4xl">
      The session DAW that stays in time
    </h1>

    <p class="text-base sm:text-lg text-[#FFFFFF]/70 max-w-2xl leading-relaxed">
      A fast audio workstation for live clip launching, sequencing, and sound design.
      Starts in 200 milliseconds, runs fully offline, and never drops a sample during playback.
    </p>

    <div class="flex flex-wrap items-center gap-4 pt-4">
      <a
        href="/download"
        class="inline-flex items-center gap-3 bg-[#86A881] text-[#0B0B0B] px-6 py-3.5 text-xs uppercase tracking-widest hover:bg-[#FFFFFF] transition-colors"
      >
        <span>&gt; Download Cassette</span>
      </a>

      <a
        href="https://github.com/saravenpi/cassette"
        target="_blank"
        rel="noreferrer"
        class="inline-flex items-center gap-3 border border-[#333333] text-[#FFFFFF] px-6 py-3.5 text-xs uppercase tracking-widest hover:border-[#86A881] hover:text-[#86A881] transition-colors"
      >
        <span>GitHub repository ↗</span>
      </a>
    </div>
  </section>

  <section class="border border-[#262626] bg-[#1F1F1F] p-4 sm:p-6 space-y-4">
    <div class="flex items-center justify-between border-b border-[#262626] pb-3 text-xs text-[#FFFFFF]/60">
      <div class="flex items-center gap-3">
        <span class="inline-block w-2.5 h-2.5 rounded-full bg-[#86A881]"></span>
        <span class="text-[#86A881] tracking-widest uppercase">Live session view</span>
        <span class="text-[#262626]">|</span>
        <span class="font-mono text-[11px]">120.0 BPM · 4/4 · 44.1 kHz</span>
      </div>
      <div class="text-[11px] uppercase tracking-widest text-[#FFFFFF]/40 hidden sm:block">
        Real-time audio: 0 xruns
      </div>
    </div>

    <div class="bg-[#0B0B0B] border border-[#262626] p-4 sm:p-6 font-mono text-xs overflow-x-auto space-y-4">
      <div class="grid grid-cols-12 gap-3 text-[10px] text-[#FFFFFF]/40 uppercase tracking-widest border-b border-[#1F1F1F] pb-2">
        <div class="col-span-3">Track</div>
        <div class="col-span-2">Type</div>
        <div class="col-span-4">Insert chain</div>
        <div class="col-span-3 text-right">Meters</div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center border-b border-[#1F1F1F]/60 pb-3">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#86A881]"></span>
          <span>01 Sub bass</span>
        </div>
        <div class="col-span-2 text-[#86A881]">Synth / MIDI</div>
        <div class="col-span-4 text-[#FFFFFF]/70 text-[11px]">PolySynth &gt; Filter &gt; Distortion</div>
        <div class="col-span-3 text-right text-[#86A881] text-[11px] font-mono">
          [||||||||||||....] -4.2 dB
        </div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center border-b border-[#1F1F1F]/60 pb-3">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#8FB8D8]"></span>
          <span>02 Drum stem</span>
        </div>
        <div class="col-span-2 text-[#8FB8D8]">Audio wave</div>
        <div class="col-span-4 text-[#FFFFFF]/70 text-[11px]">Compressor &gt; Parametric EQ &gt; Limiter</div>
        <div class="col-span-3 text-right text-[#86A881] text-[11px] font-mono">
          [||||||||||||||..] -1.8 dB
        </div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center border-b border-[#1F1F1F]/60 pb-3">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#D9B26F]"></span>
          <span>03 Lead pluck</span>
        </div>
        <div class="col-span-2 text-[#D9B26F]">Synth / MIDI</div>
        <div class="col-span-4 text-[#FFFFFF]/70 text-[11px]">PolySynth &gt; Chorus &gt; Tape Delay &gt; Reverb</div>
        <div class="col-span-3 text-right text-[#86A881] text-[11px] font-mono">
          [||||||||........] -8.4 dB
        </div>
      </div>

      <div class="grid grid-cols-12 gap-3 items-center">
        <div class="col-span-3 text-[#FFFFFF] flex items-center gap-2">
          <span class="w-2 h-2 bg-[#C87D55]"></span>
          <span>04 Master</span>
        </div>
        <div class="col-span-2 text-[#C87D55]">Bus</div>
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
          Features
        </div>
        <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Everything you need to write and perform</h2>
      </div>
      <div class="text-xs text-[#FFFFFF]/50 uppercase tracking-widest">
        Zero bloat
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      {#each features as feature}
        <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-4 hover:border-[#86A881]/50 transition-colors">
          <div class="space-y-1">
            <h3 class="text-sm font-medium tracking-wide text-[#FFFFFF]">{feature.title}</h3>
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
        DSP effects
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Built-in stereo processing suite</h2>
    </div>

    <p class="text-xs sm:text-sm text-[#FFFFFF]/70 max-w-2xl leading-relaxed">
      Clean signal processing without licensing popups or third-party wrappers. Reorder them on the fly, copy and paste effect blocks with their settings, tweak parameters with your keyboard arrows, and route sidechain compression anywhere.
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
        Stability
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Built so your audio never drops</h2>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
      <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-3">
        <div class="text-xs text-[#86A881] uppercase tracking-widest">Dedicated audio thread</div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          The rendering loop runs isolated from the interface. It owns synthesis voices, audio streams, and effect calculations, communicating through lock-free ring buffers.
        </p>
      </div>

      <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-3">
        <div class="text-xs text-[#86A881] uppercase tracking-widest">Zero garbage collection</div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          Buffers are pre-allocated upfront. Moving tracks, firing clips, or tweaking filters triggers zero runtime memory allocations on the audio path.
        </p>
      </div>

      <div class="bg-[#1F1F1F] border border-[#262626] p-6 space-y-3">
        <div class="text-xs text-[#86A881] uppercase tracking-widest">Non-blocking UI</div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          The visual interface runs as an independent surface. Heavy UI updates or animations never delay audio callbacks by even a fraction of a millisecond.
        </p>
      </div>
    </div>
  </section>

  <section id="specs" class="space-y-8 border-t border-[#262626] pt-16">
    <div class="space-y-2">
      <div class="text-xs text-[#86A881] uppercase tracking-widest">
        Technical specifications
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Engine and platform targets</h2>
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
    <div class="text-xs text-[#86A881] uppercase tracking-widest">Get started</div>
    <h2 class="text-3xl sm:text-4xl uppercase tracking-tight">Ready to make music</h2>
    <p class="text-xs sm:text-sm text-[#FFFFFF]/70 max-w-xl mx-auto leading-relaxed">
      Download the installer for your operating system or build from source. Free, open source, and lightweight.
    </p>
    <div>
      <a
        href="/download"
        class="inline-flex items-center gap-3 bg-[#86A881] text-[#0B0B0B] px-8 py-4 text-xs uppercase tracking-widest hover:bg-[#FFFFFF] transition-colors"
      >
        <span>&gt; Go to downloads</span>
      </a>
    </div>
  </section>
</div>
