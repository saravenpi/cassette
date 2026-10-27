<script lang="ts">
	/**
	 * The effect browser: one row per built-in effect, with what it does. A
	 * double-click or Enter adds it to the end of the selected track's chain, and
	 * dragging a row drops it into a gap of the device chain. Starting a drag
	 * shows the Devices view so there is a chain to drop on. A kind the track
	 * already holds is marked, since a chain takes each kind once.
	 */

	import { onDestroy, onMount } from 'svelte';

	import { EFFECT_BLURBS } from '$lib/chain';
	import { detail } from '$lib/detail.svelte';
	import { effectDrag } from '$lib/effectDrag.svelte';
	import { session } from '$lib/session.svelte';
	import type { EffectKind } from '$lib/wire';

	const trackIndex = $derived(session.snapshot?.cursor.track ?? 0);
	const track = $derived(session.snapshot?.tracks[trackIndex] ?? null);
	const catalog = $derived(session.snapshot?.effectCatalog ?? []);
	const canAdd = $derived((track?.effects.length ?? 0) < 8);

	onMount(() => {
		effectDrag.onStart = () => detail.show('devices');
	});

	onDestroy(() => {
		effectDrag.onStart = null;
		effectDrag.stop();
	});

	function add(kind: EffectKind): void {
		if (!canAdd) return;
		session.addEffect(trackIndex, kind);
		detail.show('devices');
	}

	function onKeyDown(event: KeyboardEvent, kind: EffectKind): void {
		if (event.key !== 'Enter' || !canAdd) return;
		event.preventDefault();
		event.stopPropagation();
		add(kind);
	}
</script>

<div class="pane">
	<h2 class="eyebrow">Audio effects</h2>
	{#if session.snapshot === null}
		<p class="note">connecting…</p>
	{:else}
		<ul class="list" aria-label="Effects: double-click or press Enter to add to {track?.name ?? 'the selected track'}, or drag into the chain">
			{#each catalog as spec (spec.kind)}
				<li>
					<button
						type="button"
						class="row"
						disabled={!canAdd}
						title={canAdd
							? 'Double-click or press Enter to add to the selected track, or drag into the device chain'
							: 'Track chain is full (maximum 8 effects)'}
						onpointerdown={(event) => {
							if (canAdd) effectDrag.begin(event, spec.kind, spec.name);
						}}
						ondblclick={() => add(spec.kind)}
						onkeydown={(event) => onKeyDown(event, spec.kind)}
					>
						<span class="line">
							<span class="name">{spec.name}</span>
						</span>
						<span class="blurb">{EFFECT_BLURBS[spec.kind]}</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

{#if effectDrag.kind !== null && effectDrag.ghost !== null}
	<div class="ghost" style="left: {effectDrag.ghost.x + 14}px; top: {effectDrag.ghost.y + 14}px">
		{effectDrag.name}
	</div>
{/if}

<style>
	.pane {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.eyebrow {
		flex: none;
		padding: 12px 12px 8px;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.list {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		margin: 0;
		padding: 0;
		list-style: none;
		border-top: 1px solid var(--border-subtle);
	}

	.row {
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 2px;
		width: 100%;
		height: auto;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border-subtle);
		background: transparent;
		text-align: left;
		text-transform: none;
		letter-spacing: 0.02em;
		white-space: normal;
		cursor: grab;
	}

	.row:hover:not(:disabled) {
		filter: none;
		background: var(--surface-hover);
	}

	.row:focus-visible {
		outline-offset: -2px;
	}

	.line {
		display: flex;
		align-items: baseline;
		gap: 8px;
	}

	.name {
		font-size: 12px;
		color: var(--text);
	}

	.blurb {
		font-size: 11px;
		line-height: 1.4;
		color: var(--dim);
	}

	.note {
		padding: 12px;
		font-size: 11px;
		color: var(--text-subtle);
	}

	.ghost {
		position: fixed;
		z-index: 20;
		pointer-events: none;
		padding: 4px 8px;
		font-size: 11px;
		color: var(--on-green);
		background: var(--green);
		box-shadow: var(--shadow);
	}
</style>
