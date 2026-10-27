<script lang="ts">
  import Icon from "@iconify/svelte";

  let activeTab = $state<"devices" | "pianoroll">("devices");

  const features = [
    {
      title: "Glitch-free playback",
      icon: "pixelarticons:zap",
      summary: "Zero-allocation audio loop",
      description: "The audio thread never allocates memory or locks during playback. Reorder tracks, tweak reverbs, or add clips in the middle of a set without a single dropout."
    },
    {
      title: "Modular insert chain",
      icon: "pixelarticons:sliders",
      summary: "Nine stereo effects per channel",
      description: "Each track runs its own effects chain. Chorus, tape delay, plate reverb, overdrive, bitcrusher, state-variable filter, compressor with sidechain, limiter, and 3-band EQ."
    },
    {
      title: "Fast piano roll",
      icon: "pixelarticons:grid",
      summary: "Direct note and velocity editing",
      description: "Draw and duplicate notes with velocity bars. Grid snapping scales from whole notes down to 1/64 triplets, with quantize and scale folding built right in."
    },
    {
      title: "Beat timeline",
      icon: "pixelarticons:audio-waveform",
      summary: "Sample-accurate audio and MIDI blocks",
      description: "Slice, duplicate, loop, and drag audio samples and MIDI clips. Waveforms render instantly on high-refresh displays."
    },
    {
      title: "Built-in polysynth",
      icon: "pixelarticons:keyboard-music",
      summary: "Subtractive sound engine",
      description: "Dual anti-aliased oscillators, resonant low-pass filter, ADSR envelopes, and LFO modulation. Play it with your MIDI controller or computer keyboard immediately."
    },
    {
      title: "Instant startup",
      icon: "pixelarticons:speed-fast",
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

<div class="max-w-6xl mx-auto px-4 sm:px-6 pt-12 sm:pt-16 pb-20 sm:pb-24 space-y-16 sm:space-y-24">
  <section class="space-y-6 pt-4">
    <div class="inline-flex items-center gap-2 text-xs text-[#88A888] uppercase tracking-widest">
      <span class="inline-block w-2 h-2 bg-[#88A888]"></span>
      <span>Open source audio workstation</span>
    </div>

    <h1 class="text-3xl sm:text-5xl md:text-6xl uppercase tracking-tight leading-tight sm:leading-none max-w-4xl">
      The session DAW that stays in time
    </h1>

    <p class="text-sm sm:text-lg text-[#FFFFFF]/70 max-w-2xl leading-relaxed">
      A fast audio workstation for live clip launching, sequencing, and sound design.
      Starts in 200 milliseconds, runs fully offline, and never drops a sample during playback.
    </p>

    <div class="flex flex-col sm:flex-row sm:items-center gap-3 sm:gap-4 pt-4">
      <a
        href="/download"
        class="inline-flex items-center justify-center gap-3 bg-[#88A888] text-[#000000] px-6 py-3.5 text-xs uppercase tracking-widest hover:bg-[#FFFFFF] transition-colors w-full sm:w-auto"
      >
        <Icon icon="pixelarticons:download" class="w-4 h-4 shrink-0" />
        <span>Download Cassette</span>
      </a>

      <a
        href="https://github.com/saravenpi/cassette"
        target="_blank"
        rel="noreferrer"
        class="inline-flex items-center justify-center gap-3 border border-[#333333] text-[#FFFFFF] px-6 py-3.5 text-xs uppercase tracking-widest hover:border-[#88A888] hover:text-[#88A888] transition-colors w-full sm:w-auto"
      >
        <Icon icon="pixelarticons:github" class="w-4 h-4 shrink-0" />
        <span>GitHub repository</span>
      </a>
    </div>
  </section>

  <section id="preview" class="space-y-4">
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-3 border-b border-[#262626] pb-3">
      <div class="flex items-center gap-2">
        <span class="inline-block w-2 h-2 bg-[#88A888]"></span>
        <span class="text-xs uppercase tracking-widest text-[#FFFFFF]/70">Session preview</span>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          class="inline-flex items-center gap-2 px-3 sm:px-4 py-2 text-[11px] sm:text-xs uppercase tracking-widest transition-colors {activeTab === 'devices' ? 'bg-[#88A888] text-[#000000]' : 'border border-[#262626] text-[#FFFFFF]/70 hover:text-[#FFFFFF] hover:border-[#88A888]/50'}"
          onclick={() => (activeTab = "devices")}
        >
          <Icon icon="pixelarticons:sliders" class="w-3.5 h-3.5 shrink-0" />
          <span>Devices & Instruments</span>
        </button>
        <button
          type="button"
          class="inline-flex items-center gap-2 px-3 sm:px-4 py-2 text-[11px] sm:text-xs uppercase tracking-widest transition-colors {activeTab === 'pianoroll' ? 'bg-[#88A888] text-[#000000]' : 'border border-[#262626] text-[#FFFFFF]/70 hover:text-[#FFFFFF] hover:border-[#88A888]/50'}"
          onclick={() => (activeTab = "pianoroll")}
        >
          <Icon icon="pixelarticons:grid" class="w-3.5 h-3.5 shrink-0" />
          <span>Piano Roll & Effects</span>
        </button>
      </div>
    </div>

    <div class="border border-[#262626] bg-[#000000] p-2 sm:p-4 overflow-hidden">
      {#if activeTab === "devices"}
        <img
          src="/screenshot-devices.png"
          alt="Cassette DAW session view with synth browser, arrangement timeline, and stereo DSP devices rack"
          class="w-full h-auto block object-contain shadow-2xl"
          loading="eager"
        />
      {:else}
        <img
          src="/screenshot-piano-roll.png"
          alt="Cassette DAW piano roll with velocity editing, MIDI notes, and audio effects rack"
          class="w-full h-auto block object-contain shadow-2xl"
          loading="eager"
        />
      {/if}
    </div>
  </section>

  <section id="features" class="space-y-8 sm:space-y-12">
    <div class="flex flex-col sm:flex-row sm:items-end justify-between gap-4 border-b border-[#262626] pb-6">
      <div class="space-y-2">
        <div class="inline-flex items-center gap-2 text-xs text-[#88A888] uppercase tracking-widest">
          <Icon icon="pixelarticons:sliders" class="w-4 h-4" />
          <span>Features</span>
        </div>
        <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Everything you need to write and perform</h2>
      </div>
      <div class="text-xs text-[#FFFFFF]/50 uppercase tracking-widest">
        Zero bloat
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 sm:gap-6">
      {#each features as feature}
        <div class="bg-[#1F1F1F] border border-[#262626] p-5 sm:p-6 space-y-4 hover:border-[#88A888]/50 transition-colors">
          <div class="flex items-start justify-between gap-4">
            <div class="space-y-1">
              <h3 class="text-sm font-medium tracking-wide text-[#FFFFFF]">{feature.title}</h3>
              <div class="text-xs text-[#88A888] tracking-wide">{feature.summary}</div>
            </div>
            <Icon icon={feature.icon} class="w-5 h-5 text-[#88A888] shrink-0" />
          </div>
          <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
            {feature.description}
          </p>
        </div>
      {/each}
    </div>
  </section>

  <section id="effects" class="space-y-6 sm:space-y-8 border-t border-[#262626] pt-12 sm:pt-16">
    <div class="space-y-2">
      <div class="inline-flex items-center gap-2 text-xs text-[#88A888] uppercase tracking-widest">
        <Icon icon="pixelarticons:audio-device" class="w-4 h-4" />
        <span>DSP effects</span>
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Built-in stereo processing suite</h2>
    </div>

    <p class="text-xs sm:text-sm text-[#FFFFFF]/70 max-w-2xl leading-relaxed">
      Clean signal processing without licensing popups or third-party wrappers. Reorder them on the fly, copy and paste effect blocks with their settings, tweak parameters with your keyboard arrows, and route sidechain compression anywhere.
    </p>

    <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-3 gap-2.5 sm:gap-3 pt-2">
      {#each effectsList as effect}
        <div class="border border-[#262626] bg-[#1F1F1F] p-3 sm:p-4 text-center hover:border-[#88A888]/40 transition-colors flex items-center justify-center gap-1.5 sm:gap-2">
          <Icon icon="pixelarticons:sliders" class="w-3.5 h-3.5 text-[#88A888]/70 shrink-0" />
          <span class="text-[11px] sm:text-xs uppercase tracking-widest text-[#FFFFFF] truncate">{effect}</span>
        </div>
      {/each}
    </div>
  </section>

  <section id="architecture" class="space-y-6 sm:space-y-8 border-t border-[#262626] pt-12 sm:pt-16">
    <div class="space-y-2">
      <div class="inline-flex items-center gap-2 text-xs text-[#88A888] uppercase tracking-widest">
        <Icon icon="pixelarticons:code" class="w-4 h-4" />
        <span>Stability</span>
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Built so your audio never drops</h2>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-3 gap-4 sm:gap-6">
      <div class="bg-[#1F1F1F] border border-[#262626] p-5 sm:p-6 space-y-3">
        <div class="flex items-center justify-between">
          <div class="text-xs text-[#88A888] uppercase tracking-widest">Dedicated audio thread</div>
          <Icon icon="pixelarticons:cpu" class="w-4 h-4 text-[#88A888]" />
        </div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          The rendering loop runs isolated from the interface. It owns synthesis voices, audio streams, and effect calculations, communicating through lock-free ring buffers.
        </p>
      </div>

      <div class="bg-[#1F1F1F] border border-[#262626] p-5 sm:p-6 space-y-3">
        <div class="flex items-center justify-between">
          <div class="text-xs text-[#88A888] uppercase tracking-widest">Zero garbage collection</div>
          <Icon icon="pixelarticons:server" class="w-4 h-4 text-[#88A888]" />
        </div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          Buffers are pre-allocated upfront. Moving tracks, firing clips, or tweaking filters triggers zero runtime memory allocations on the audio path.
        </p>
      </div>

      <div class="bg-[#1F1F1F] border border-[#262626] p-5 sm:p-6 space-y-3">
        <div class="flex items-center justify-between">
          <div class="text-xs text-[#88A888] uppercase tracking-widest">Non-blocking UI</div>
          <Icon icon="pixelarticons:device-laptop" class="w-4 h-4 text-[#88A888]" />
        </div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          The visual interface runs as an independent surface. Heavy UI updates or animations never delay audio callbacks by even a fraction of a millisecond.
        </p>
      </div>
    </div>
  </section>

  <section id="specs" class="space-y-6 sm:space-y-8 border-t border-[#262626] pt-12 sm:pt-16">
    <div class="space-y-2">
      <div class="inline-flex items-center gap-2 text-xs text-[#88A888] uppercase tracking-widest">
        <Icon icon="pixelarticons:checklist" class="w-4 h-4" />
        <span>Technical specifications</span>
      </div>
      <h2 class="text-2xl sm:text-3xl uppercase tracking-tight">Engine and platform targets</h2>
    </div>

    <div class="border border-[#262626] divide-y divide-[#262626] bg-[#1F1F1F]">
      {#each specs as spec}
        <div class="p-4 sm:p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-1.5 sm:gap-2 text-xs">
          <span class="text-[#FFFFFF]/50 uppercase tracking-widest">{spec.label}</span>
          <span class="text-[#FFFFFF] text-[11px] sm:text-xs break-words sm:break-normal">{spec.value}</span>
        </div>
      {/each}
    </div>
  </section>

  <section id="get-started">
    <div class="bg-[#88A888] text-[#000000] p-6 sm:p-8 md:p-12 flex flex-col justify-between space-y-6 sm:space-y-8">
      <div class="space-y-3 sm:space-y-4">
        <div class="text-xs uppercase tracking-widest text-[#000000]/70">
          Get started
        </div>
        <h2 class="text-2xl sm:text-4xl md:text-5xl uppercase leading-tight font-normal">
          Ready to make music
        </h2>
        <p class="text-xs sm:text-sm leading-relaxed text-[#000000]/80 max-w-2xl">
          Download the installer for your operating system or build from source. Free, open source, and lightweight.
        </p>
      </div>

      <div class="pt-2">
        <a
          href="/download"
          class="inline-flex items-center justify-center gap-2.5 w-full sm:w-auto bg-[#000000] text-[#FFFFFF] hover:bg-[#FFFFFF] hover:text-[#000000] px-6 sm:px-8 py-3.5 sm:py-4 text-xs sm:text-sm uppercase tracking-widest text-center no-underline transition-colors leading-none"
        >
          <Icon
            icon="pixelarticons:download"
            class="w-5 h-5 shrink-0 translate-y-[1px]"
          />
          <span class="leading-none">Go to downloads</span>
        </a>
      </div>
    </div>
  </section>
</div>