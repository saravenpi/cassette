<script lang="ts">
  import Icon from "@iconify/svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  let scrollY = $state(0);
  let isScrolled = $derived(scrollY > 20);
  let mobileMenuOpen = $state(false);
</script>

<svelte:window
  bind:scrollY
  onkeydown={(e) => {
    if (e.key === "Escape" && mobileMenuOpen) {
      mobileMenuOpen = false;
    }
  }}
/>

<div class="h-14 md:h-16 w-full" aria-hidden="true"></div>

<header
  class="reveal-nav fixed top-0 left-0 right-0 z-40 flex justify-center pointer-events-none"
>
  <div
    class="pointer-events-auto transition-all duration-300 ease-out {isScrolled
      ? 'w-[calc(100%-2rem)] md:w-[calc(100%-4rem)] max-w-6xl mt-2 md:mt-3 px-4 md:px-6 py-3 bg-[#000000] border border-[#262626] shadow-2xl'
      : 'w-full max-w-full mt-0 px-6 md:px-12 py-4 bg-[#000000] border-b border-[#262626]'}"
  >
    <div
      class="max-w-6xl mx-auto flex items-center justify-between gap-4 text-[11px] uppercase tracking-widest leading-none"
    >
      <a href="/" class="flex items-center gap-2.5 no-underline text-[#FFFFFF] hover:text-[#88A888] transition-colors">
        <img
          src="/logo.svg"
          alt="CASSETTE"
          class="h-6 w-6 block"
        />
        <span class="tracking-widest">CASSETTE</span>
      </a>

      <div class="hidden md:flex items-center gap-6">
        <a
          href="/#preview"
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-2 py-1 leading-none transition-colors"
          >PREVIEW</a
        >
        <a
          href="/#features"
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-2 py-1 leading-none transition-colors"
          >FEATURES</a
        >
        <a
          href="/#effects"
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-2 py-1 leading-none transition-colors"
          >EFFECTS</a
        >
        <a
          href="/#specs"
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-2 py-1 leading-none transition-colors"
          >SPECS</a
        >
        <a
          href="https://github.com/saravenpi/cassette"
          target="_blank"
          rel="noreferrer"
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-2 py-1 leading-none transition-colors inline-flex items-center gap-1.5"
        >
          <span>SOURCE</span>
          <Icon icon="pixelarticons:external-link" class="w-3.5 h-3.5 opacity-70" />
        </a>
        <a
          href="/download"
          class="no-underline bg-[#88A888] text-[#000000] px-3 py-1.5 leading-none hover:bg-[#FFFFFF] transition-colors inline-flex items-center gap-2"
        >
          <Icon icon="pixelarticons:download" class="w-3.5 h-3.5 shrink-0" />
          <span>GET CASSETTE</span>
        </a>
      </div>

      <button
        type="button"
        onclick={() => (mobileMenuOpen = !mobileMenuOpen)}
        class="md:hidden flex items-center justify-center p-1 text-[#FFFFFF] hover:text-[#88A888] cursor-pointer"
        aria-label="Toggle navigation menu"
        aria-expanded={mobileMenuOpen}
      >
        <Icon icon="pixelarticons:menu" class="w-6 h-6" />
      </button>
    </div>
  </div>
</header>

{#if mobileMenuOpen}
  <button
    type="button"
    class="fixed inset-0 bg-[#000000]/80 backdrop-blur-sm z-50 md:hidden border-none p-0 cursor-pointer"
    transition:fade={{ duration: 200 }}
    onclick={() => (mobileMenuOpen = false)}
    aria-label="Close navigation overlay"
    tabindex="-1"
  ></button>

  <aside
    class="fixed top-0 right-0 bottom-0 w-72 max-w-[85vw] bg-[#000000] z-50 p-6 flex flex-col justify-between shadow-2xl md:hidden border-l border-[#262626]"
    aria-label="Mobile navigation"
    transition:fly={{ x: 288, duration: 300, easing: cubicOut, opacity: 1 }}
  >
    <div class="space-y-8">
      <div class="flex items-center justify-between pb-4 border-b border-[#262626]">
        <div class="flex items-center gap-2">
          <img src="/logo.svg" alt="CASSETTE" class="h-5 w-5" />
          <span class="text-xs uppercase tracking-widest text-[#FFFFFF]">CASSETTE</span>
        </div>
        <button
          type="button"
          onclick={() => (mobileMenuOpen = false)}
          class="p-1 text-[#FFFFFF] hover:text-[#88A888] cursor-pointer"
          aria-label="Close menu"
        >
          <Icon icon="pixelarticons:close" class="w-6 h-6" />
        </button>
      </div>

      <div class="flex flex-col space-y-2 text-[12px] uppercase tracking-widest">
        <a
          href="/#preview"
          onclick={() => (mobileMenuOpen = false)}
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-3 py-2 transition-colors"
          >PREVIEW</a
        >
        <a
          href="/#features"
          onclick={() => (mobileMenuOpen = false)}
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-3 py-2 transition-colors"
          >FEATURES</a
        >
        <a
          href="/#effects"
          onclick={() => (mobileMenuOpen = false)}
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-3 py-2 transition-colors"
          >EFFECTS</a
        >
        <a
          href="/#specs"
          onclick={() => (mobileMenuOpen = false)}
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-3 py-2 transition-colors"
          >SPECS</a
        >
        <a
          href="https://github.com/saravenpi/cassette"
          target="_blank"
          rel="noreferrer"
          onclick={() => (mobileMenuOpen = false)}
          class="text-[#FFFFFF] no-underline hover:bg-[#88A888] hover:text-[#000000] px-3 py-2 transition-colors inline-flex items-center justify-between"
        >
          <span>SOURCE</span>
          <Icon icon="pixelarticons:external-link" class="w-3.5 h-3.5 opacity-70" />
        </a>
        <a
          href="/download"
          onclick={() => (mobileMenuOpen = false)}
          class="no-underline bg-[#88A888] text-[#000000] px-3 py-2.5 hover:bg-[#FFFFFF] transition-colors text-center mt-4 inline-flex items-center justify-center gap-2"
        >
          <Icon icon="pixelarticons:download" class="w-4 h-4" />
          <span>GET CASSETTE</span>
        </a>
      </div>
    </div>

    <div class="pt-6 border-t border-[#262626]">
      <a
        href="https://github.com/saravenpi/cassette"
        target="_blank"
        rel="noreferrer"
        class="inline-flex items-center gap-2 text-[11px] text-[#FFFFFF]/70 hover:text-[#88A888] no-underline tracking-wider leading-none"
      >
        <Icon icon="pixelarticons:github" class="w-4 h-4 shrink-0" />
        <span class="leading-none">saravenpi/cassette</span>
      </a>
    </div>
  </aside>
{/if}
