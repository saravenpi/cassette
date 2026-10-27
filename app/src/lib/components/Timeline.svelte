<script lang="ts">
	/**
	 * The main window: a ruler, one lane and one header per track, the playhead,
	 * and one scroll surface for all of it. The track lanes sit on the left and the
	 * header column sits on the right, sticky so it stays visible while scrolling.
	 * A hover line follows the pointer across the lanes and marks the beat a
	 * split or a click would land on; Alt frees it from the grid.
	 */

	import { onDestroy, tick } from 'svelte';

	import {
		HEADER_WIDTH,
		anchoredScroll,
		MAX_ROW_HEIGHT,
		MIN_ROW_HEIGHT,
		ROW_HEIGHT,
		RULER_HEIGHT,
		barsFor,
		beatsToPx,
		clampRowHeight,
		formatPrecise,
		gridStep,
		pruneHeights,
		snapBeats,
		zoomByWheel
	} from '$lib/geometry';
	import { laneUnder } from '$lib/drag.svelte';
	import { emptyTimelineMenu, markAt, selectionMenu } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
	import { formatRangeLength, rangeLength } from '$lib/selection';
	import { selection } from '$lib/selection.svelte';
	import { session } from '$lib/session.svelte';
	import Playhead from './Playhead.svelte';
	import Ruler from './Ruler.svelte';
	import TrackHeader from './TrackHeader.svelte';
	import TrackLane from './TrackLane.svelte';

	const ROW_KEY_STEP = 8;

	let scroller = $state<HTMLDivElement | null>(null);
	let trackHeights = $state<Record<number, number>>({});
	let cancelResize: (() => void) | null = null;
	let pointer: { x: number; y: number; free: boolean } | null = null;

	const snapshot = $derived(session.snapshot);
	const tracks = $derived(snapshot?.tracks ?? []);
	const beatsPerBar = $derived(snapshot?.beatsPerBar ?? 4);
	const bars = $derived(barsFor(snapshot?.lengthBeats ?? 0, beatsPerBar, snapshot?.bars ?? 0));
	const contentWidth = $derived(beatsToPx(bars * beatsPerBar, session.zoom));
	const barWidth = $derived(beatsToPx(beatsPerBar, session.zoom));
	const gridLabel = $derived.by(() => {
		const step = gridStep(session.zoom);
		return step >= 1 ? String(step) : `1/${Math.round(1 / step)}`;
	});

	function getHeight(index: number): number {
		return trackHeights[index] ?? ROW_HEIGHT;
	}

	const selRange = $derived(selection.range);
	const selLayout = $derived.by(() => {
		if (selRange === null) return null;
		let top = RULER_HEIGHT;
		for (let i = 0; i < selRange.firstTrack && i < tracks.length; i += 1) {
			top += getHeight(i);
		}
		let height = 0;
		for (let i = selRange.firstTrack; i <= selRange.lastTrack && i < tracks.length; i += 1) {
			height += getHeight(i);
		}
		const left = beatsToPx(selRange.startBeats, session.zoom);
		const width = beatsToPx(selRange.endBeats - selRange.startBeats, session.zoom);
		return { top, height, left, width };
	});

	function onGridPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		const target = event.target;
		if (!(target instanceof Element)) return;
		if (
			target.closest(
				'[data-block], .row-resize-handle, .selection-handle, button, input, .ruler, .corner, .slot'
			)
		) {
			return;
		}
		const lane = laneUnder(event.clientX, event.clientY, session.zoom);
		if (lane === null) return;
		event.preventDefault();

		const step = gridStep(session.zoom);
		const snappedBeats = snapBeats(lane.beats, step, event.altKey);

		if (event.shiftKey) {
			selection.extend(lane.track, snappedBeats);
			session.dispatch({ selectTrack: { track: lane.track } });
			return;
		}

		selection.beginDrag(lane.track, snappedBeats);
		let moved = false;
		const startX = event.clientX;
		const startY = event.clientY;

		const onMove = (e: PointerEvent) => {
			if (!moved && (Math.abs(e.clientX - startX) > 3 || Math.abs(e.clientY - startY) > 3)) {
				moved = true;
			}
			const curLane = laneUnder(e.clientX, e.clientY, session.zoom);
			if (curLane !== null) {
				const curBeats = snapBeats(curLane.beats, gridStep(session.zoom), e.altKey);
				selection.updateDrag(curLane.track, curBeats);
			}
		};

		const onUp = (e: PointerEvent) => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
			window.removeEventListener('pointercancel', onUp);
			const drawn = selection.finishDrag();
			if (!drawn && !moved) {
				selection.clear();
				const endLane = laneUnder(e.clientX, e.clientY, session.zoom) ?? lane;
				const endBeats = snapBeats(endLane.beats, gridStep(session.zoom), e.altKey);
				session.dispatch({ selectTrack: { track: endLane.track } });
				session.dispatch({ seek: { beats: Math.max(0, endBeats) } });
			}
		};

		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
		window.addEventListener('pointercancel', onUp);
	}

	function startSelectionResize(event: PointerEvent, edge: 'start' | 'end'): void {
		if (event.button !== 0) return;
		event.preventDefault();
		event.stopPropagation();
		selection.beginResize(edge);

		const onMove = (e: PointerEvent) => {
			const lane = laneUnder(e.clientX, e.clientY, session.zoom);
			if (lane !== null) {
				const beats = snapBeats(lane.beats, gridStep(session.zoom), e.altKey);
				selection.updateResize(beats);
			}
		};

		const onUp = () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
			window.removeEventListener('pointercancel', onUp);
			selection.finishResize();
		};

		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
		window.addEventListener('pointercancel', onUp);
	}

	function onSelectionContextMenu(event: MouseEvent): void {
		if (selRange === null) return;
		event.preventDefault();
		event.stopPropagation();
		menu.open(
			event.clientX,
			event.clientY,
			selectionMenu(
				session,
				selRange,
				session.snapshot?.clipboard ?? null,
				session.snapshot?.cursor.track ?? 0,
				session.snapshot?.transportPositionBeats ?? 0,
				session.snapshot?.looping ?? false,
				() => selection.clear()
			)
		);
	}

	function openAddTrackMenu(event: MouseEvent): void {
		const button = event.currentTarget;
		if (!(button instanceof HTMLElement)) return;
		const rect = button.getBoundingClientRect();
		const entries = emptyTimelineMenu(session, tracks.length);
		menu.open(rect.left, rect.bottom + 2, entries, button);
	}

	function onEmptySpaceContextMenu(event: MouseEvent): void {
		if (
			event.target instanceof Element &&
			(event.target.closest('[data-block]') ||
				event.target.closest('.row-resize-handle') ||
				event.target.closest('.selection-band') ||
				event.target.closest('.ruler') ||
				event.target.closest('.corner'))
		) {
			return;
		}
		event.preventDefault();
		menu.open(event.clientX, event.clientY, emptyTimelineMenu(session, tracks.length));
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

	function blockUnder(x: number, y: number, track: number): number | null {
		const hit = document.elementFromPoint(x, y);
		const element = hit instanceof Element ? hit.closest('[data-block]') : null;
		if (!(element instanceof HTMLElement)) return null;
		const index = Number(element.dataset.block);
		return Number(element.dataset.blockTrack) === track && Number.isInteger(index) ? index : null;
	}

	function trackHover(): void {
		const lane = pointer === null ? null : laneUnder(pointer.x, pointer.y, session.zoom);
		if (pointer === null || lane === null) {
			session.hover = null;
			return;
		}
		const block = blockUnder(pointer.x, pointer.y, lane.track);
		const view = block === null ? undefined : tracks[lane.track]?.blocks[block];
		session.hover = { beats: markAt(view, lane.beats, gridStep(session.zoom), pointer.free), track: lane.track, block };
	}

	function onPointerMove(event: PointerEvent): void {
		pointer = { x: event.clientX, y: event.clientY, free: event.altKey };
		trackHover();
	}

	function onPointerLeave(): void {
		pointer = null;
		session.hover = null;
	}

	async function onWheel(event: WheelEvent): Promise<void> {
		if (!(event.ctrlKey || event.metaKey) || scroller === null) return;
		event.preventDefault();
		const element = scroller;
		const x = event.clientX - element.getBoundingClientRect().left;
		const from = session.zoom;
		const delta = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaY * 33 : event.deltaY;
		session.zoomTo(zoomByWheel(from, delta));
		if (session.zoom === from) return;
		const left = anchoredScroll(element.scrollLeft, x, from, session.zoom);
		await tick();
		element.scrollLeft = left;
	}

	$effect(() => {
		const element = scroller;
		if (element === null) return;
		const listener = (event: WheelEvent) => void onWheel(event);
		element.addEventListener('wheel', listener, { passive: false });
		return () => element.removeEventListener('wheel', listener);
	});

	onDestroy(() => {
		cancelResize?.();
		session.hover = null;
	});

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
	<div
		class="scroller"
		role="presentation"
		bind:this={scroller}
		onpointermove={onPointerMove}
		onpointerleave={onPointerLeave}
		onscroll={trackHover}
	>
		<div
			class="grid"
			role="presentation"
			style="width: {HEADER_WIDTH + contentWidth}px"
			onpointerdown={onGridPointerDown}
		>
			<div class="row ruler-row" style="height: {RULER_HEIGHT}px">
				<Ruler pxPerBeat={session.zoom} {beatsPerBar} {bars} width={contentWidth} />
				<div class="corner" style="width: {HEADER_WIDTH}px">
					<span class="hover-at">
						{#if selRange !== null}
							{formatRangeLength(rangeLength(selRange), beatsPerBar)}
						{:else if session.hover !== null}
							{formatPrecise(session.hover.beats, beatsPerBar)}
						{/if}
					</span>
					<div class="zoom-row">
						<span class="zoom-label">Grid {gridLabel}</span>
						<button class="zoom" aria-label="Zoom out" onclick={() => session.setZoom(-1)}>
							&minus;
						</button>
						<span class="zoom-value">{Math.round(session.zoom)}</span>
						<button class="zoom" aria-label="Zoom in" onclick={() => session.setZoom(1)}>+</button>
					</div>
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

			<div class="row add-track-row" role="presentation" oncontextmenu={onEmptySpaceContextMenu}>
				<div class="empty-lane" style="width: {contentWidth}px"></div>
				<div class="slot add-slot" style="width: {HEADER_WIDTH}px">
					<button
						type="button"
						class="add-track-btn"
						aria-label="Add track"
						title={tracks.length >= 16 ? 'Track limit reached (16 tracks)' : 'Add track'}
						disabled={tracks.length >= 16}
						onclick={openAddTrackMenu}
					>
						+
					</button>
				</div>
			</div>

			<div
				class="empty-space"
				style="width: {HEADER_WIDTH + contentWidth}px"
				role="presentation"
				oncontextmenu={onEmptySpaceContextMenu}
			></div>

			{#if selLayout !== null && selRange !== null}
				<div
					class="selection-band"
					role="region"
					aria-label="Time selection: tracks {selRange.firstTrack + 1} to {selRange.lastTrack + 1}, beats {selRange.startBeats + 1} to {selRange.endBeats + 1}"
					style="top: {selLayout.top}px; left: {selLayout.left}px; width: {selLayout.width}px; height: {selLayout.height}px"
					oncontextmenu={onSelectionContextMenu}
				>
					<button
						type="button"
						class="selection-handle start"
						aria-label="Adjust selection start"
						onpointerdown={(e) => startSelectionResize(e, 'start')}
					></button>
					<button
						type="button"
						class="selection-handle end"
						aria-label="Adjust selection end"
						onpointerdown={(e) => startSelectionResize(e, 'end')}
					></button>
				</div>
			{/if}

			<Playhead
				beats={snapshot?.transportPositionBeats ?? 0}
				pxPerBeat={session.zoom}
				offset={0}
				top={RULER_HEIGHT}
			/>

			{#if session.hover !== null}
				<div
					class="hover-line"
					style="left: {beatsToPx(session.hover.beats, session.zoom)}px; top: {RULER_HEIGHT}px; height: calc(100% - {RULER_HEIGHT}px)"
				></div>
			{/if}
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
		background: var(--surface);
		border-bottom: 1px solid var(--border-subtle);
	}

	.corner,
	.slot {
		position: sticky;
		right: 0;
		flex: none;
		z-index: 3;
		background: var(--surface);
		border-left: 1px solid var(--border-subtle);
	}

	.corner {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		padding: 8px 10px 6px;
	}

	.hover-at {
		min-height: 16px;
		font-size: 13px;
		letter-spacing: 0.04em;
		color: var(--text);
		font-variant-numeric: tabular-nums;
	}

	.hover-line {
		position: absolute;
		width: 0;
		border-left: 1px dashed var(--text-subtle);
		pointer-events: none;
		z-index: 2;
	}

	.zoom-row {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	.zoom-label {
		margin-right: auto;
		font-size: 10px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
		font-variant-numeric: tabular-nums;
	}

	.zoom {
		width: 20px;
		height: 20px;
		padding: 0;
		font-size: 12px;
		letter-spacing: 0;
		background: transparent;
		color: var(--text-subtle);
	}

	.zoom:hover:not(:disabled) {
		filter: none;
		color: var(--text);
		background: var(--surface-hover);
	}

	.zoom-value {
		min-width: 24px;
		font-size: 10px;
		text-align: center;
		color: var(--text-subtle);
		font-variant-numeric: tabular-nums;
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
		background: var(--green);
		opacity: 0.5;
	}

	.add-track-row {
		height: 32px;
	}

	.empty-lane {
		flex: none;
		background: var(--bg);
		border-bottom: 1px solid var(--border-subtle);
	}

	.add-slot {
		align-items: center;
		justify-content: center;
		border-bottom: 1px solid var(--border-subtle);
	}

	.add-track-btn {
		width: 22px;
		height: 22px;
		padding: 0;
		font-size: 14px;
		letter-spacing: 0;
		background: transparent;
		border: 1px dashed var(--border);
		color: var(--text-subtle);
	}

	.add-track-btn:hover:not(:disabled) {
		filter: none;
		border-color: var(--green);
		color: var(--green);
		background: var(--surface-hover);
	}

	.add-track-btn:disabled {
		opacity: 0.3;
	}

	.empty-space {
		min-height: 120px;
		background: var(--bg);
	}

	.selection-band {
		position: absolute;
		border: 1px solid var(--green);
		background: color-mix(in srgb, var(--green) 18%, transparent);
		pointer-events: auto;
		z-index: 4;
		box-sizing: border-box;
	}

	.selection-handle {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 6px;
		padding: 0;
		border: 0;
		background: transparent;
		cursor: ew-resize;
		z-index: 5;
	}

	.selection-handle.start {
		left: -3px;
	}

	.selection-handle.end {
		right: -3px;
	}

	.selection-handle:hover,
	.selection-handle:focus-visible {
		filter: none;
		background: var(--green);
		opacity: 0.7;
	}
</style>
