<script lang="ts">
  import "../app.css";
  import Icon, { addCollection } from "@iconify/svelte";
  import pixelarticons from "@iconify-json/pixelarticons/icons.json";

  addCollection(pixelarticons);

  let { children } = $props();
</script>

<div class="min-h-screen bg-[#0B0B0B] text-[#FFFFFF] flex flex-col font-['Undefined',monospace]">
  <header class="border-b border-[#262626] sticky top-0 z-50 bg-[#0B0B0B]/90 backdrop-blur">
    <div class="max-w-6xl mx-auto px-6 py-4 flex items-center justify-between gap-6 text-xs uppercase tracking-widest">
      <a href="/" class="flex items-center gap-3 text-[#FFFFFF] hover:text-[#86A881] transition-colors">
        <img src="/logo.svg" alt="Cassette" class="h-6 w-6 block" />
        <span class="tracking-widest">CASSETTE</span>
      </a>

      <nav class="hidden md:flex items-center gap-8 text-[11px] text-[#FFFFFF]/70">
        <a href="/" class="hover:text-[#FFFFFF] transition-colors">OVERVIEW</a>
        <a href="/#features" class="hover:text-[#FFFFFF] transition-colors">FEATURES</a>
        <a href="/#effects" class="hover:text-[#FFFFFF] transition-colors">EFFECTS</a>
        <a href="/#specs" class="hover:text-[#FFFFFF] transition-colors">SPECS</a>
        <a
          href="https://github.com/saravenpi/cassette"
          target="_blank"
          rel="noreferrer"
          class="inline-flex items-center gap-1.5 hover:text-[#FFFFFF] transition-colors"
        >
          <span>SOURCE</span>
          <Icon icon="pixelarticons:external-link" class="w-3.5 h-3.5 opacity-70" />
        </a>
      </nav>

      <div class="flex items-center gap-4">
        <a
          href="/download"
          class="inline-flex items-center gap-2 bg-[#86A881] text-[#0B0B0B] px-4 py-2 text-xs uppercase tracking-widest hover:bg-[#FFFFFF] transition-colors"
        >
          <Icon icon="pixelarticons:download" class="w-3.5 h-3.5" />
          <span>&gt; GET CASSETTE</span>
        </a>
      </div>
    </div>
  </header>

  <main class="flex-1">
    {@render children()}
  </main>

  <footer class="border-t border-[#262626] mt-24 py-12 text-xs text-[#FFFFFF]/60">
    <div class="max-w-6xl mx-auto px-6 flex flex-col md:flex-row items-center justify-between gap-6">
      <div class="flex items-center gap-3">
        <img src="/logo.svg" alt="Cassette" class="h-5 w-5 opacity-70" />
        <span>CASSETTE DAW</span>
        <span class="text-[#262626]">|</span>
        <span>OPEN SOURCE SESSION WORKSTATION</span>
      </div>
      <div class="flex items-center gap-6 text-[11px] uppercase tracking-widest">
        <a href="/download" class="inline-flex items-center gap-1.5 hover:text-[#86A881] transition-colors">
          <Icon icon="pixelarticons:download" class="w-3.5 h-3.5" />
          <span>DOWNLOADS</span>
        </a>
        <a
          href="https://github.com/saravenpi/cassette/releases"
          target="_blank"
          rel="noreferrer"
          class="inline-flex items-center gap-1.5 hover:text-[#86A881] transition-colors"
        >
          <Icon icon="pixelarticons:external-link" class="w-3.5 h-3.5" />
          <span>RELEASES</span>
        </a>
        <a
          href="https://github.com/saravenpi/cassette"
          target="_blank"
          rel="noreferrer"
          class="inline-flex items-center gap-1.5 hover:text-[#86A881] transition-colors"
        >
          <Icon icon="pixelarticons:github" class="w-3.5 h-3.5" />
          <span>GITHUB</span>
        </a>
      </div>
    </div>
  </footer>
</div>
