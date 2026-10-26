<script lang="ts">
	/**
	 * The main window: a ruler, one lane and one header per track, the playhead,
	 * and one scroll surface for all of it. The track lanes sit on the left and the
	 * header column sits on the right, sticky so it stays visible while scrolling.
	 */

	import { onDestroy } from 'svelte';

	import {
		HEADER_WIDTH,
		MAX_ROW_HEIGHT,
		MIN_ROW_HEIGHT,
		ROW_HEIGHT,
		RULER_HEIGHT,
		barsFor,
		beatsToPx,
		clampRowHeight,
		pruneHeights
	} from '$lib/geometry';
	import { session } from '$lib/session.svelte';
	import Playhead from './Playhead.svelte';
	import Ruler from './Ruler.svelte';
	import TrackHeader from './TrackHeader.svelte';
	import TrackLane from './TrackLane.svelte';

	const ROW_KEY_STEP = 8;

	let scroller = $state<HTMLDivElement | null>(null);
	let trackHeights = $state<Record<number, number>>({});
	let cancelResize: (() => void) | null = null;

	const snapshot = $derived(session.snapshot);
	const tracks = $derived(snapshot?.tracks ?? []);
	const beatsPerBar = $derived(snapshot?.beatsPerBar ?? 4);
	const bars = $derived(barsFor(snapshot?.lengthBeats ?? 0, beatsPerBar, snapshot?.bars ?? 0));
	const contentWidth = $derived(beatsToPx(bars * beatsPerBar, session.zoom));
	const barWidth = $derived(beatsToPx(beatsPerBar, session.zoom));

	function getHeight(index: number): number {
		return trackHeights[index] ?? ROW_HEIGHT;
	}

	function startRowResize(event: PointerEvent, index: number, initialHeight: number): void {
		if (event.button !== 0) return;
		event.preventDefault();
		const startY = event.clientY;

		cancelResize?.();

		const onMove = (e: PointerEvent) => {
			trackHeights[index] = clampRowHeight(initialHeight + e.clientY - startY);
		};

		const onEnd = () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onEnd);
			window.removeEventListener('pointercancel', onEnd);
			cancelResize = null;
		};

		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onEnd);
		window.addEventListener('pointercancel', onEnd);
		cancelResize = onEnd;
	}

	function onResizeKey(event: KeyboardEvent, index: number, height: number): void {
		const step = event.key === 'ArrowDown' ? ROW_KEY_STEP : event.key === 'ArrowUp' ? -ROW_KEY_STEP : 0;
		if (step === 0) return;
		event.preventDefault();
		trackHeights[index] = clampRowHeight(height + step);
	}

	function resetRowHeight(index: number): void {
		trackHeights[index] = ROW_HEIGHT;
	}

	onDestroy(() => cancelResize?.());

	$effect(() => {
		const pruned = pruneHeights(trackHeights, tracks.length);
		if (Object.keys(pruned).length !== Object.keys(trackHeights).length) trackHeights = pruned;
	});

	$effect(() => {
		if (session.scrollRequest === 0) return;
		scroller?.scrollTo({ left: 0, top: 0 });
	});
</script>

<section class="timeline">
	<div class="scroller" bind:this={scroller}>
		<div class="grid" style="width: {HEADER_WIDTH + contentWidth}px">
			<div class="row ruler-row" style="height: {RULER_HEIGHT}px">
				<Ruler pxPerBeat={session.zoom} {beatsPerBar} {bars} width={contentWidth} />
				<div class="corner" style="width: {HEADER_WIDTH}px">
					<span class="zoom-label">zoom</span>
					<button class="zoom" aria-label="Zoom out" onclick={() => session.setZoom(-1)}>
						&minus;
					</button>
					<span class="zoom-value">{Math.round(session.zoom)}</span>
					<button class="zoom" aria-label="Zoom in" onclick={() => session.setZoom(1)}>+</button>
				</div>
			</div>

			{#each tracks as track, index (index)}
				{@const trackHeight = getHeight(index)}
				<div class="row track-row" style="height: {trackHeight}px">
					<TrackLane {track} {index} width={contentWidth} {barWidth} height={trackHeight} />
					<div class="slot" style="width: {HEADER_WIDTH}px">
						<TrackHeader {track} {index} height={trackHeight} />
					</div>
					<div
						class="row-resize-handle"
						role="slider"
						aria-orientation="vertical"
						aria-label="Resize height of {track.name}"
						aria-valuenow={trackHeight}
						aria-valuemin={MIN_ROW_HEIGHT}
						aria-valuemax={MAX_ROW_HEIGHT}
						tabindex="0"
						onkeydown={(e) => onResizeKey(e, index, trackHeight)}
						onpointerdown={(e) => startRowResize(e, index, trackHeight)}
						ondblclick={() => resetRowHeight(index)}
					></div>
				</div>
			{/each}

			<Playhead
				beats={snapshot?.transportPositionBeats ?? 0}
				pxPerBeat={session.zoom}
				offset={0}
				top={RULER_HEIGHT}
			/>
		</div>
	</div>
</section>

<style>
	.timeline {
		flex: 1;
		min-width: 0;
		display: flex;
	}

	.scroller {
		flex: 1;
		min-width: 0;
		overflow: auto;
		background: var(--bg);
	}

	.grid {
		position: relative;
		min-height: 100%;
	}

	.row {
		display: flex;
		position: relative;
	}

	.ruler-row {
		position: sticky;
		top: 0;
		z-index: 5;
		background: var(--panel);
		border-bottom: 1px solid var(--line);
	}

	.corner,
	.slot {
		position: sticky;
		right: 0;
		flex: none;
		z-index: 3;
		background: var(--panel);
		border-left: 1px solid var(--line);
	}

	.corner {
		display: flex;
		align-items: flex-end;
		justify-content: flex-end;
		gap: 4px;
		padding: 0 6px 4px;
	}

	.zoom-label {
		font-size: 9px;
		color: var(--dim);
		margin-right: auto;
		align-self: center;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.zoom {
		padding: 0 5px;
		font-size: 10px;
		line-height: 14px;
		height: 16px;
		border-radius: 0;
	}

	.zoom-value {
		font-size: 9px;
		color: var(--dim);
		font-variant-numeric: tabular-nums;
		min-width: 18px;
		text-align: center;
		align-self: center;
	}

	.slot {
		display: flex;
	}

	.row-resize-handle {
		position: absolute;
		bottom: -3px;
		left: 0;
		right: 0;
		height: 6px;
		cursor: row-resize;
		z-index: 8;
	}

	.row-resize-handle:hover,
	.row-resize-handle:focus-visible {
		background: var(--accent);
		opacity: 0.5;
	}
</style>
