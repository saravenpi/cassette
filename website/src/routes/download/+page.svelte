<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "@iconify/svelte";

  let { data } = $props();

  const base = "https://github.com/saravenpi/cassette/releases/latest/download/";
  const releasesUrl = "https://github.com/saravenpi/cassette/releases/latest";

  type Os = "windows" | "macos" | "linux";

  const platforms: {
    os: Os;
    name: string;
    icon: string;
    files: { label: string; asset: string }[];
    notes: string[];
  }[] = [
    {
      os: "windows",
      name: "WINDOWS",
      icon: "pixelarticons:device-laptop",
      files: [
        {
          label: "Windows Installer (.exe)",
          asset: "Cassette-windows-x64-setup.exe"
        }
      ],
      notes: [
        "Unsigned build: Windows SmartScreen may trigger on first launch.",
        'Click "More info", then choose "Run anyway".',
        "Per-user installation requires no administrator elevation."
      ]
    },
    {
      os: "macos",
      name: "MACOS",
      icon: "pixelarticons:devices",
      files: [
        {
          label: "Disk Image Universal (.dmg)",
          asset: "Cassette-macos-universal.dmg"
        }
      ],
      notes: [
        "Universal binary for Apple Silicon (M1/M2/M3/M4) and Intel.",
        "Mount the DMG and drag Cassette into your Applications folder.",
        'Right-click the app icon, select "Open", then confirm "Open".'
      ]
    },
    {
      os: "linux",
      name: "LINUX",
      icon: "pixelarticons:terminal",
      files: [
        {
          label: "Standalone AppImage (.AppImage)",
          asset: "Cassette-linux-x64.AppImage"
        },
        {
          label: "Debian / Ubuntu Package (.deb)",
          asset: "Cassette-linux-x64.deb"
        }
      ],
      notes: [
        "AppImage: run chmod +x Cassette-linux-x64.AppImage then execute.",
        "Debian: run sudo apt install ./Cassette-linux-x64.deb",
        "Supports PipeWire, PulseAudio, and ALSA audio drivers."
      ]
    }
  ];

  let detected = $state<Os | null>(null);

  onMount(() => {
    const ua = navigator.userAgent;
    if (/windows/i.test(ua)) {
      detected = "windows";
    } else if (/android/i.test(ua)) {
      detected = null;
    } else if (/mac|iphone|ipad/i.test(ua)) {
      detected = "macos";
    } else if (/linux|x11/i.test(ua)) {
      detected = "linux";
    }
  });

  const ordered = $derived(
    detected
      ? [
          ...platforms.filter((p) => p.os === detected),
          ...platforms.filter((p) => p.os !== detected)
        ]
      : platforms
  );

  const version = $derived<string>(data.version || "0.21.0");
  const notesUrl = $derived<string>(data.url ?? releasesUrl);
</script>

<svelte:head>
  <title>Download Cassette | Session Audio Workstation</title>
  <meta
    name="description"
    content="Download Cassette for Windows, macOS, and Linux. Free, lightweight open source audio workstation."
  />
</svelte:head>

<div class="max-w-6xl mx-auto px-4 sm:px-6 pt-10 sm:pt-12 pb-20 sm:pb-24 space-y-12 sm:space-y-16">
  <section class="space-y-4">
    <div class="inline-flex items-center gap-2 text-xs text-[#88A888] uppercase tracking-widest">
      <Icon icon="pixelarticons:keyboard-music" class="w-4 h-4 text-[#88A888]" />
      <span>CASSETTE DAW · V{version}</span>
    </div>
    <h1 class="text-3xl md:text-5xl uppercase tracking-tight">DOWNLOAD</h1>
    <p class="text-sm text-[#FFFFFF]/70 max-w-2xl leading-relaxed">
      Standalone desktop packages for Windows, macOS, and Linux. Each installer
      includes the complete real-time audio engine, instruments, insert effects, and editor.
    </p>
    <div>
      <a
        href={notesUrl}
        target="_blank"
        rel="noreferrer"
        class="inline-flex items-center gap-1.5 text-xs uppercase tracking-widest text-[#88A888] hover:text-[#FFFFFF] transition-colors"
      >
        <span>RELEASE NOTES ON GITHUB</span>
        <Icon icon="pixelarticons:external-link" class="w-3.5 h-3.5" />
      </a>
    </div>
  </section>

  <section class="space-y-6">
    {#each ordered as platform, i (platform.os)}
      {@const primary = detected !== null && i === 0}
      <div
        class="bg-[#1F1F1F] p-5 sm:p-6 md:p-8 grid grid-cols-1 md:grid-cols-12 gap-6 md:gap-8 {primary
          ? 'border border-[#88A888]'
          : 'border border-[#262626]'}"
      >
        <div class="md:col-span-5 space-y-4">
          <div class="flex items-center gap-3">
            <Icon icon={platform.icon} class="w-5 h-5 text-[#88A888]" />
            <h2 class="text-lg uppercase tracking-wide">{platform.name}</h2>
            {#if primary}
              <span class="bg-[#88A888] text-[#000000] px-2 py-0.5 text-[10px] uppercase tracking-widest">
                YOUR SYSTEM
              </span>
            {/if}
          </div>

          <div class="flex flex-col gap-3">
            {#each platform.files as file}
              <a
                href={base + file.asset}
                class="inline-flex items-center justify-between gap-3 px-4 sm:px-5 py-3 text-xs uppercase tracking-widest transition-colors {primary
                  ? 'bg-[#88A888] text-[#000000] hover:bg-[#FFFFFF]'
                  : 'border border-[#333333] text-[#FFFFFF] hover:border-[#88A888] hover:text-[#88A888]'}"
              >
                <span class="inline-flex items-center gap-2 truncate">
                  <Icon icon="pixelarticons:download" class="w-4 h-4 shrink-0" />
                  <span class="truncate">{file.label}</span>
                </span>
                <span class="text-[10px] opacity-60 shrink-0">X64</span>
              </a>
            {/each}
          </div>
        </div>

        <div class="md:col-span-7 space-y-3">
          <div class="text-[11px] uppercase tracking-widest text-[#88A888]">
            FIRST RUN & VERIFICATION
          </div>
          <ul class="space-y-2 text-xs text-[#FFFFFF]/75">
            {#each platform.notes as note}
              <li class="leading-relaxed flex items-start gap-2">
                <Icon icon="pixelarticons:check" class="w-3.5 h-3.5 text-[#88A888] shrink-0 mt-0.5" />
                <span>{note}</span>
              </li>
            {/each}
          </ul>
        </div>
      </div>
    {/each}
  </section>

  <section class="space-y-6 pt-4 border-t border-[#262626]">
    <div class="flex flex-col md:flex-row md:items-end justify-between gap-3">
      <div class="flex items-center gap-2">
        <Icon icon="pixelarticons:debug-check" class="w-5 h-5 text-[#88A888]" />
        <h2 class="text-xl md:text-2xl uppercase tracking-wide">VERIFY INTEGRITY</h2>
      </div>
      <span class="text-[11px] uppercase tracking-widest text-[#FFFFFF]/50">
        SHA-256 SECURE HASHES
      </span>
    </div>

    <div class="bg-[#1F1F1F] border border-[#262626] p-4 sm:p-6 space-y-4 text-xs text-[#FFFFFF]/80">
      <p class="leading-relaxed">
        Verify that your downloaded binary matches the official release artifacts:
      </p>
      <div class="bg-[#000000] border border-[#262626] p-3 sm:p-4 text-[11px] space-y-2 overflow-x-auto text-[#FFFFFF]/90">
        <div><span class="text-[#88A888]"># Windows (PowerShell)</span></div>
        <div>Get-FileHash Cassette-windows-x64-setup.exe -Algorithm SHA256</div>
        <div class="pt-2"><span class="text-[#88A888]"># macOS & Linux</span></div>
        <div>sha256sum Cassette-linux-x64.AppImage</div>
        <div>shasum -a 256 Cassette-macos-universal.dmg</div>
      </div>
    </div>
  </section>

  <section class="space-y-6 pt-4 border-t border-[#262626]">
    <div class="flex flex-col md:flex-row md:items-end justify-between gap-3">
      <div class="flex items-center gap-2">
        <Icon icon="pixelarticons:code" class="w-5 h-5 text-[#88A888]" />
        <h2 class="text-xl md:text-2xl uppercase tracking-wide">BUILD FROM SOURCE</h2>
      </div>
      <span class="text-[11px] uppercase tracking-widest text-[#FFFFFF]/50">
        DEVELOPER WORKFLOW
      </span>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4 sm:gap-6">
      <div class="bg-[#1F1F1F] border border-[#262626] p-5 sm:p-6 space-y-4">
        <div class="flex items-center justify-between">
          <div class="text-[11px] uppercase tracking-widest text-[#88A888]">
            Audio engine tests (Cargo)
          </div>
          <Icon icon="pixelarticons:code" class="w-4 h-4 text-[#88A888]" />
        </div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          Run the test suite across the audio core, synthesis models, and real-time buffers:
        </p>
        <div class="bg-[#000000] border border-[#262626] p-3 sm:p-4 text-xs overflow-x-auto">
          <div class="text-[#88A888]">$ git clone https://github.com/saravenpi/cassette.git</div>
          <div class="text-[#FFFFFF]/90">$ cd cassette</div>
          <div class="text-[#FFFFFF]/90">$ cargo test</div>
        </div>
      </div>

      <div class="bg-[#1F1F1F] border border-[#262626] p-5 sm:p-6 space-y-4">
        <div class="flex items-center justify-between">
          <div class="text-[11px] uppercase tracking-widest text-[#88A888]">
            Desktop build (Mise)
          </div>
          <Icon icon="pixelarticons:terminal" class="w-4 h-4 text-[#88A888]" />
        </div>
        <p class="text-xs text-[#FFFFFF]/70 leading-relaxed">
          Compile and launch the full desktop application in development mode:
        </p>
        <div class="bg-[#000000] border border-[#262626] p-3 sm:p-4 text-xs overflow-x-auto">
          <div class="text-[#88A888]">$ mise run deps</div>
          <div class="text-[#FFFFFF]/90">$ mise run dev</div>
          <div class="text-[#FFFFFF]/90">$ mise run build</div>
        </div>
      </div>
    </div>
  </section>

  <section class="border-t border-[#262626] pt-8 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 text-xs text-[#FFFFFF]/60">
    <div>
      Need historical versions or nightly assets?
    </div>
    <a
      href="https://github.com/saravenpi/cassette/releases"
      target="_blank"
      rel="noreferrer"
      class="inline-flex items-center gap-1.5 text-[#88A888] hover:text-[#FFFFFF] uppercase tracking-widest transition-colors"
    >
      <span>ALL RELEASES ON GITHUB</span>
      <Icon icon="pixelarticons:external-link" class="w-3.5 h-3.5" />
    </a>
  </section>
</div>