<script lang="ts">
	/**
	 * One block on a lane: its label, the waveform of the sample it plays or a
	 * preview of its notes, its selection and the two drags a pointer can start on
	 * it. A double-click on a MIDI block opens it in the piano roll. The inline
	 * rename field is the engine's, so typing is routed through the wire, not the
	 * browser.
	 */

	import { loadWaveform } from '$lib/api';
	import { beatUnder, blockMenu } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
	import { MIN_BLOCK_PX, beatsToPx, pxToBeats, waveformPoints } from '$lib/geometry';
	import { trackColor } from '$lib/palette';
	import { previewRange, velocityOpacity } from '$lib/pianoRoll';
	import { session } from '$lib/session.svelte';
	import type { BlockView } from '$lib/wire';

	const WAVE_HEIGHT = 16;

	interface Props {
		block: BlockView;
		track: number;
		index: number;
		startBeats: number;
		lengthBeats: number;
		dragging: boolean;
		colorIndex: number;
	}

	let { block, track, index, startBeats, lengthBeats, dragging, colorIndex }: Props = $props();

	let peaks = $state<number[] | null>(null);

	const left = $derived(beatsToPx(startBeats, session.zoom));
	const width = $derived(Math.max(beatsToPx(lengthBeats, session.zoom), MIN_BLOCK_PX));
	const renaming = $derived(session.snapshot?.renaming === true && block.selected);
	const range = $derived(previewRange(block.notes));

	$effect(() => {
		const source = block.kind === 'audio' ? block.source : '';
		if (source === '') {
			peaks = null;
			return;
		}
		let live = true;
		void loadWaveform(source)
			.then((value) => {
				if (live) peaks = value;
			})
			.catch(() => {
				if (live) peaks = null;
			});
		return () => {
			live = false;
		};
	});

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.stopPropagation();
		const element = event.currentTarget;
		if (!(element instanceof HTMLElement)) return;
		const grab = pxToBeats(event.clientX - element.getBoundingClientRect().left, session.zoom);
		element.setPointerCapture(event.pointerId);
		session.dispatch({ selectBlock: { track, block: index } });
		session.drag.beginMove(track, index, block, grab);
	}

	function onResizePointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.stopPropagation();
		const element = event.currentTarget;
		if (element instanceof HTMLElement) element.setPointerCapture(event.pointerId);
		session.dispatch({ selectBlock: { track, block: index } });
		session.drag.beginResize(track, index, block);
	}

	function onDoubleClick(): void {
		if (block.kind === 'midi') session.openRoll(track, index);
	}

	function onContextMenu(event: MouseEvent): void {
		event.preventDefault();
		event.stopPropagation();
		const element = event.currentTarget;
		if (!(element instanceof HTMLElement)) return;
		const at = beatUnder(block.startBeats, event.clientX - element.getBoundingClientRect().left, session.zoom);
		session.dispatch({ selectBlock: { track, block: index } });
		menu.open(event.clientX, event.clientY, blockMenu(session, track, index, block, at));
	}
</script>

<div
	class="block"
	class:selected={block.selected}
	class:dragging
	role="listitem"
	data-block
	style="left: {left}px; width: {width}px; --fill: {trackColor(colorIndex)}"
	onpointerdown={onPointerDown}
	ondblclick={onDoubleClick}
	oncontextmenu={onContextMenu}
>
	{#if block.kind === 'audio' && peaks !== null}
		<svg
			class="wave"
			viewBox="0 0 {peaks.length - 1} {WAVE_HEIGHT}"
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			<polygon points={waveformPoints(peaks, WAVE_HEIGHT)} />
		</svg>
	{/if}

	{#if block.kind === 'midi' && block.notes.length > 0}
		<svg
			class="wave"
			viewBox="0 0 {Math.max(lengthBeats, 0.001)} {range.rows}"
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			{#each block.notes as note, position (position)}
				<rect
					class="bar"
					x={note.startBeats}
					y={range.top - note.pitch}
					width={note.lengthBeats}
					height="1"
					opacity={velocityOpacity(note.velocity)}
				/>
			{/each}
		</svg>
	{/if}

	{#if renaming}
		<input class="rename" value={block.name} readonly aria-label="Block name" />
	{:else}
		<span class="name">{block.name}</span>
	{/if}

	<button
		type="button"
		class="handle"
		aria-label="Resize {block.name}"
		onpointerdown={onResizePointerDown}
	></button>
</div>

<style>
	.block {
		position: absolute;
		top: 2px;
		bottom: 2px;
		height: calc(100% - 4px);
		display: flex;
		align-items: center;
		overflow: hidden;
		border: 1px solid var(--line);
		border-left: 3px solid var(--fill);
		border-radius: 0;
		background: color-mix(in srgb, var(--fill) 26%, var(--panel));
		font-size: 10px;
		cursor: grab;
		user-select: none;
		z-index: 1;
	}

	.block.selected {
		border-color: var(--text);
		box-shadow: inset 0 0 0 1px var(--text);
		z-index: 2;
	}

	.block.dragging {
		opacity: 0.85;
		cursor: grabbing;
	}

	.wave {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}

	.wave polygon {
		fill: color-mix(in srgb, var(--fill) 80%, transparent);
	}

	.wave .bar {
		fill: color-mix(in srgb, var(--fill) 85%, var(--text));
	}

	.name {
		position: relative;
		padding: 0 12px 0 5px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text);
	}

	.rename {
		position: relative;
		width: calc(100% - 8px);
		margin-left: 3px;
		padding: 0 3px;
		height: 14px;
		font: inherit;
		color: var(--head);
		background: var(--bg);
		border: 1px solid var(--head);
		border-radius: 0;
	}

	.handle {
		position: absolute;
		top: 0;
		right: 0;
		width: 7px;
		height: 100%;
		padding: 0;
		border: 0;
		border-radius: 0;
		background: transparent;
		cursor: ew-resize;
	}

	.handle:hover {
		background: color-mix(in srgb, var(--text) 30%, transparent);
	}
</style>
