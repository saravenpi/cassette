<script lang="ts">
	/**
	 * The detail pane under the timeline, after Ableton's Detail View. It has no
	 * tabs: selecting a track shows that track's devices, and selecting a MIDI
	 * clip shows the piano roll on it. The header names what is shown, with the
	 * track name (and the block name for a clip) and a hide button; Shift+Tab
	 * still switches the view. The top edge drags to resize the pane, or takes the arrow keys while
	 * focused, between 160 px and 60% of the window. The detail store keeps the
	 * height and the open state across sessions. Enter on a button in the pane
	 * stays with the button, so it never also adds a block.
	 */

	import { onDestroy } from 'svelte';

	import { RESIZE_BIG_STEP, RESIZE_STEP, MIN_HEIGHT, MAX_SHARE, clampHeight } from '$lib/detail';
	import { detail } from '$lib/detail.svelte';
	import { session } from '$lib/session.svelte';
	import { MASTER_TRACK, type TrackView } from '$lib/wire';
	import AudioClipView from './AudioClipView.svelte';
	import DeviceChain from './DeviceChain.svelte';
	import PianoRoll from './PianoRoll.svelte';

	let windowHeight = $state(typeof window === 'undefined' ? 800 : window.innerHeight);
	let resize: { y: number; height: number } | null = null;

	const height = $derived(clampHeight(detail.height, windowHeight));
	const maxHeight = $derived(Math.max(MIN_HEIGHT, Math.floor(windowHeight * MAX_SHARE)));
	const isMaster = $derived(session.selectedTrack === MASTER_TRACK);
	const trackIndex = $derived(session.snapshot?.cursor.track ?? 0);
	const track = $derived(session.snapshot?.tracks[trackIndex] ?? null);
	const masterTrack = $derived<TrackView | null>(
		session.snapshot
			? {
					name: 'Master',
					kind: 'audio',
					preset: '',
					waveform: 'sine',
					gain: 1,
					octave: 0,
					attack: 0,
					decay: 0,
					sustain: 1,
					release: 0,
					volume: session.snapshot.masterVolume ?? 0.85,
					pan: 0,
					muted: false,
					solo: false,
					level: session.snapshot.masterLevel ?? 0,
					colorIndex: 0,
					notesRevision: 0,
					blocks: [],
					effects: session.snapshot.masterEffects ?? [],
					instrumentEnabled: false,
					cutoff: 1,
					resonance: 0,
					detune: 0,
					sub: 0
				}
			: null
	);
	const activeTrack = $derived(isMaster ? masterTrack : track);
	const activeTrackIndex = $derived(isMaster ? MASTER_TRACK : trackIndex);
	const roll = $derived(session.roll);
	const rollTrack = $derived(roll === null ? null : (session.snapshot?.tracks[roll.track] ?? null));
	const rollBlock = $derived(roll === null || rollTrack === null ? null : (rollTrack.blocks[roll.block] ?? null));
	const shownTrack = $derived(detail.view === 'clip' && rollTrack !== null ? rollTrack : activeTrack);

	onDestroy(() => stopResize());

	function onResizeDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.preventDefault();
		resize = { y: event.clientY, height };
		window.addEventListener('pointermove', onResizeMove);
		window.addEventListener('pointerup', stopResize);
		window.addEventListener('pointercancel', stopResize);
	}

	function onResizeMove(event: PointerEvent): void {
		if (resize === null) return;
		detail.setHeight(clampHeight(resize.height + resize.y - event.clientY, windowHeight));
	}

	function stopResize(): void {
		resize = null;
		window.removeEventListener('pointermove', onResizeMove);
		window.removeEventListener('pointerup', stopResize);
		window.removeEventListener('pointercancel', stopResize);
	}

	function onResizeKey(event: KeyboardEvent): void {
		const step = event.shiftKey ? RESIZE_BIG_STEP : RESIZE_STEP;
		let next: number | null = null;
		if (event.key === 'ArrowUp') next = height + step;
		else if (event.key === 'ArrowDown') next = height - step;
		else if (event.key === 'Home') next = MIN_HEIGHT;
		else if (event.key === 'End') next = maxHeight;
		if (next === null) return;
		event.preventDefault();
		event.stopPropagation();
		detail.setHeight(clampHeight(next, windowHeight));
	}

	/**
	 * Attach a listener outside the template. A focusable separator is an
	 * interactive widget in WAI-ARIA, but the template linter only counts the
	 * widget roles it knows, so the handle takes its listeners this way.
	 */
	function listen<K extends keyof HTMLElementEventMap>(
		type: K,
		handler: (event: HTMLElementEventMap[K]) => void
	): (node: HTMLElement) => () => void {
		return (node) => {
			node.addEventListener(type, handler);
			return () => node.removeEventListener(type, handler);
		};
	}

	/** Put the resize handle in the tab order. */
	function focusable(node: HTMLElement): void {
		node.tabIndex = 0;
	}

	function keepEnter(event: KeyboardEvent): void {
		if (event.key === 'Enter' && event.target instanceof HTMLButtonElement) event.stopPropagation();
	}
</script>

<svelte:window bind:innerHeight={windowHeight} />

<section class="detail" aria-label="Detail" {@attach listen('keydown', keepEnter)}>
	{#if detail.open}
		<div
			class="handle"
			role="separator"
			aria-orientation="horizontal"
			aria-label="Resize the detail pane"
			aria-valuemin={MIN_HEIGHT}
			aria-valuemax={maxHeight}
			aria-valuenow={height}
			{@attach focusable}
			{@attach listen('pointerdown', onResizeDown)}
			{@attach listen('keydown', onResizeKey)}
		></div>
	{/if}

	<header class="bar">
		<span class="kind">{detail.view === 'clip' && rollBlock !== null ? 'Clip' : 'Devices'}</span>
		<span class="eyebrow">{shownTrack?.name ?? ''}</span>
		{#if detail.view === 'clip' && rollBlock !== null}
			<span class="title">{rollBlock.name}</span>
		{/if}
		<button
			type="button"
			class="icon hide"
			aria-expanded={detail.open}
			aria-controls="detail-body"
			aria-label={detail.open ? 'Hide the detail pane' : 'Show the detail pane'}
			title={detail.open ? 'Hide' : 'Show'}
			onclick={() => detail.setOpen(!detail.open)}
		>
			{detail.open ? '▾' : '▴'}
		</button>
	</header>

	{#if detail.open}
		<div class="body" id="detail-body" role="region" aria-label="{detail.view === 'clip' ? 'Clip' : 'Devices'}" style="height: {height}px">
			{#if detail.view === 'clip'}
				{#if roll !== null && rollBlock !== null && rollTrack !== null}
					{#if rollBlock.kind === 'audio'}
						<AudioClipView track={roll.track} blockIndex={roll.block} block={rollBlock} trackView={rollTrack} />
					{:else}
						{#key roll.serial}
							<PianoRoll />
						{/key}
					{/if}
				{:else}
					<p class="empty">Click a clip to inspect or edit it</p>
				{/if}
			{:else if activeTrack === null}
				<p class="empty">connecting…</p>
			{:else}
				<DeviceChain track={activeTrack} index={activeTrackIndex} />
			{/if}
		</div>
	{/if}
</section>

<style>
	.detail {
		position: relative;
		flex: none;
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--border);
		background: var(--bg);
	}

	.handle {
		position: absolute;
		top: -4px;
		left: 0;
		right: 0;
		z-index: 3;
		height: 7px;
		cursor: ns-resize;
		touch-action: none;
	}

	.handle:hover,
	.handle:focus-visible {
		outline: none;
		background: var(--green-soft);
		box-shadow: inset 0 -1px 0 var(--green), inset 0 1px 0 var(--green);
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 32px;
		flex: none;
		padding: 0 8px 0 4px;
		border-bottom: 1px solid var(--border-subtle);
		background: var(--surface);
	}

	.kind {
		padding: 0 10px 0 0;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text);
	}

	.eyebrow {
		padding-left: 6px;
		border-left: 1px solid var(--border-subtle);
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
		white-space: nowrap;
	}

	.title {
		font-size: 12px;
		color: var(--text);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.icon {
		width: 26px;
		height: 26px;
		padding: 0;
		background: transparent;
		color: var(--text-subtle);
		font-size: 12px;
		letter-spacing: 0;
	}

	.hide {
		margin-left: auto;
	}

	.icon:hover:not(:disabled) {
		filter: none;
		background: var(--surface-hover);
		color: var(--text);
	}

	.body {
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	.empty {
		margin: auto;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--dim);
	}
</style>
