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
				{selected.name} plays <strong>{selected.preset}</strong> on a
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
		padding: 8px 10px;
	}

	.note {
		margin: 0 0 10px;
		font-size: 11px;
		line-height: 1.45;
		color: var(--dim);
	}

	code {
		font-family: ui-monospace, monospace;
		font-size: 10px;
		color: var(--text);
		background: var(--panel-2);
		padding: 0 3px;
		border-radius: 2px;
	}

	.voices {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.voice {
		width: 100%;
		text-align: left;
		font-size: 11px;
		padding: 3px 8px;
		border-radius: 2px;
	}

	.voice.on {
		border-color: var(--head);
		color: var(--head);
	}

	.current {
		margin: 10px 0 0;
		font-size: 11px;
		line-height: 1.45;
		color: var(--dim);
	}

	.current strong {
		color: var(--text);
	}
</style>
