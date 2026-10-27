<script lang="ts">
  import "../app.css";
  import { addCollection } from "@iconify/svelte";
  import pixelarticons from "@iconify-json/pixelarticons/icons.json";
  import Icon from "@iconify/svelte";
  import TopNav from "$lib/components/TopNav.svelte";

  addCollection(pixelarticons);

  let { children } = $props();
</script>

<div class="min-h-screen bg-[#000000] text-[#FFFFFF] flex flex-col font-['Undefined',monospace]">
  <TopNav />

  <main class="flex-1">
    {@render children()}
  </main>

  <footer class="border-t border-[#262626] mt-24 py-12 text-xs text-[#FFFFFF]/60">
    <div class="max-w-6xl mx-auto px-4 sm:px-6 flex flex-col md:flex-row items-center justify-between gap-6">
      <div class="flex items-center gap-3">
        <Icon icon="pixelarticons:keyboard-music" class="w-5 h-5 text-[#88A888] shrink-0" />
        <span>CASSETTE DAW</span>
        <span class="text-[#262626]">|</span>
        <span>OPEN SOURCE SESSION WORKSTATION</span>
      </div>
      <div class="flex flex-wrap items-center justify-center gap-6 text-[11px] uppercase tracking-widest">
        <a href="/download" class="inline-flex items-center gap-1.5 hover:text-[#88A888] transition-colors">
          <Icon icon="pixelarticons:download" class="w-3.5 h-3.5" />
          <span>DOWNLOADS</span>
        </a>
        <a
          href="https://github.com/saravenpi/cassette/releases"
          target="_blank"
          rel="noreferrer"
          class="inline-flex items-center gap-1.5 hover:text-[#88A888] transition-colors"
        >
          <Icon icon="pixelarticons:external-link" class="w-3.5 h-3.5" />
          <span>RELEASES</span>
        </a>
        <a
          href="https://github.com/saravenpi/cassette"
          target="_blank"
          rel="noreferrer"
          class="inline-flex items-center gap-1.5 hover:text-[#88A888] transition-colors"
        >
          <Icon icon="pixelarticons:github" class="w-3.5 h-3.5" />
          <span>GITHUB</span>
        </a>
      </div>
    </div>
  </footer>
</div>
