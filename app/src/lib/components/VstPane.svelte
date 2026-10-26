<script lang="ts">
	/**
	 * The VST pane. The engine hosts no plugin format, so this pane lists what
	 * actually exists, the built-in voices, and says where a host would attach.
	 */

	import { session } from '$lib/session.svelte';

	const snapshot = $derived(session.snapshot);
	const presets = $derived(snapshot?.presets ?? []);
	const trackIndex = $derived(snapshot?.cursor.track ?? 0);
	const selected = $derived.by(() => {
		const current = session.snapshot;
		return current ? (current.tracks[current.cursor.track] ?? null) : null;
	});
</script>

<div class="pane">
	<h2 class="eyebrow">Instruments</h2>
	<p class="note">
		This engine hosts no plugin format. There is no VST, VST3, AU or CLAP loader in the crate:
		a track plays one of the built-in voices below, and picking one sends <code>setPreset</code>.
		Hosting would attach behind the engine's <code>Processor</code> trait, on the far side of the
		command ring, which is where the mixer and the sample pool already sit.
	</p>

	{#if snapshot === null}
		<p class="note">connecting…</p>
	{:else}
		<ul class="voices">
			{#each presets as preset (preset)}
				<li>
					<button
						class="voice"
						class:on={selected?.preset === preset}
						aria-pressed={selected?.preset === preset}
						onclick={() => session.dispatch({ setPreset: { track: trackIndex, preset } })}
					>
						{preset}
					</button>
				</li>
			{/each}
		</ul>

		{#if selected !== null}
			<p class="current">
				{selected.name} plays <span class="strong">{selected.preset}</span> on a
				{selected.waveform.toLowerCase()} oscillator. The waveform follows the preset; the
				envelope is on the effects pane.
			</p>
		{/if}
	{/if}
</div>

<style>
	.pane {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		overflow-y: auto;
		padding: 12px 14px;
	}

	.eyebrow {
		margin: 0 0 10px;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.note {
		margin: 0 0 12px;
		font-size: 12px;
		line-height: 1.5;
		color: var(--dim);
	}

	code {
		font-family: inherit;
		font-size: 11px;
		color: var(--olive);
	}

	.voices {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
		border-top: 1px solid var(--border-subtle);
	}

	.voices li {
		border-bottom: 1px solid var(--border-subtle);
	}

	.voice {
		justify-content: flex-start;
		width: 100%;
		height: 32px;
		padding: 0 10px;
		background: transparent;
		color: var(--muted);
		font-size: 12px;
		letter-spacing: 0.02em;
		text-transform: none;
		text-align: left;
	}

	.voice:hover:not(:disabled) {
		filter: none;
		background: var(--surface-hover);
		color: var(--text);
	}

	.voice.on {
		background: var(--green-soft);
		color: var(--green);
	}

	.current {
		margin: 12px 0 0;
		font-size: 12px;
		line-height: 1.5;
		color: var(--dim);
	}

	.strong {
		color: var(--text);
	}
</style>
