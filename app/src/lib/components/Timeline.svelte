<script lang="ts">
	/**
	 * The main window: a ruler, one lane and one header per track, the playhead,
	 * and one scroll surface for all of it. The header column is sticky so it stays
	 * put while the song scrolls under it.
	 */

	import { HEADER_WIDTH, ROW_HEIGHT, RULER_HEIGHT, barsFor, beatsToPx } from '$lib/geometry';
	import { session } from '$lib/session.svelte';
	import Playhead from './Playhead.svelte';
	import Ruler from './Ruler.svelte';
	import TrackHeader from './TrackHeader.svelte';
	import TrackLane from './TrackLane.svelte';

	let scroller = $state<HTMLDivElement | null>(null);

	const snapshot = $derived(session.snapshot);
	const tracks = $derived(snapshot?.tracks ?? []);
	const beatsPerBar = $derived(snapshot?.beatsPerBar ?? 4);
	const bars = $derived(barsFor(snapshot?.lengthBeats ?? 0, beatsPerBar, snapshot?.bars ?? 0));
	const contentWidth = $derived(beatsToPx(bars * beatsPerBar, session.zoom));
	const barWidth = $derived(beatsToPx(beatsPerBar, session.zoom));

	$effect(() => {
		if (session.scrollRequest === 0) return;
		scroller?.scrollTo({ left: 0, top: 0 });
	});
</script>

<section class="timeline">
	<div class="scroller" bind:this={scroller}>
		<div class="grid" style="width: {HEADER_WIDTH + contentWidth}px">
			<div class="row ruler-row" style="height: {RULER_HEIGHT}px">
				<div class="corner">
					<span class="zoom-label">zoom</span>
					<button class="zoom" aria-label="Zoom out" onclick={() => session.setZoom(-1)}>
						&minus;
					</button>
					<span class="zoom-value">{Math.round(session.zoom)}</span>
					<button class="zoom" aria-label="Zoom in" onclick={() => session.setZoom(1)}>+</button>
				</div>
				<Ruler pxPerBeat={session.zoom} {beatsPerBar} {bars} />
			</div>

			{#each tracks as track, index (index)}
				<div class="row" style="height: {ROW_HEIGHT}px">
					<div class="slot" style="width: {HEADER_WIDTH}px">
						<TrackHeader {track} {index} />
					</div>
					<TrackLane {track} {index} width={contentWidth} {barWidth} />
				</div>
			{/each}

			<Playhead
				beats={snapshot?.transportPositionBeats ?? 0}
				pxPerBeat={session.zoom}
				offset={HEADER_WIDTH}
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
		left: 0;
		flex: none;
		z-index: 3;
		background: var(--panel);
	}

	.corner {
		display: flex;
		align-items: flex-end;
		justify-content: flex-end;
		gap: 4px;
		padding: 0 6px 4px;
		border-right: 1px solid var(--line);
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
		border-radius: 2px;
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
</style>
