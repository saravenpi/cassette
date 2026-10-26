<script lang="ts">
	/**
	 * The effects pane: what is actually live on the selected track, which is its
	 * voice, plus an honest line about the insert effects that are not built yet.
	 */

	import { session } from '$lib/session.svelte';
	import VoiceEditor from './VoiceEditor.svelte';

	const index = $derived(session.snapshot?.cursor.track ?? 0);
	const selected = $derived.by(() => {
		const snapshot = session.snapshot;
		return snapshot ? (snapshot.tracks[snapshot.cursor.track] ?? null) : null;
	});
</script>

<div class="pane">
	<h2 class="eyebrow">Effects</h2>
	<p class="note">
		Insert effects are not implemented: no reverb, delay, filter or compressor is compiled into
		the engine yet, so there are no controls for them here.
	</p>

	{#if selected === null}
		<p class="note">connecting…</p>
	{:else if selected.kind === 'audio'}
		<p class="note">
			{selected.name} is an audio track: it plays the sample on its block straight through its
			mixer strip, which is the volume and pan faders in the track header.
		</p>
	{:else}
		<p class="note">
			{selected.name} plays <span class="strong">{selected.preset}</span> on a
			{selected.waveform.toLowerCase()} oscillator, so the waveform is fixed by the preset and
			nothing here changes it.
		</p>
		<VoiceEditor track={selected} {index} />
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
		padding-bottom: 12px;
		border-bottom: 1px solid var(--border-subtle);
		font-size: 12px;
		line-height: 1.5;
		color: var(--dim);
	}

	.strong {
		color: var(--text);
	}
</style>
