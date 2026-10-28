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
	import {
		MIN_BLOCK_PX,
		beatsToPx,
		gridStep,
		pxToBeats,
		waveWindow,
		waveformPoints
	} from '$lib/geometry';
	import { trackColor } from '$lib/palette';
	import { previewRange, velocityOpacity } from '$lib/pianoRoll';
	import { selection } from '$lib/selection.svelte';
	import { detail } from '$lib/detail.svelte';
	import { session } from '$lib/session.svelte';
	import type { BlockView, Waveform } from '$lib/wire';

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

	let wave = $state<Waveform | null>(null);
	let lastDownAt = 0;
	const fillColor = $derived(trackColor(colorIndex));

	const left = $derived(beatsToPx(startBeats, session.zoom));
	const width = $derived(Math.max(beatsToPx(lengthBeats, session.zoom), MIN_BLOCK_PX));
	const renaming = $derived(session.snapshot?.renaming === true && block.selected);
	const range = $derived(previewRange(block.notes));
	const bpm = $derived(session.snapshot?.bpm ?? 120);
	const editing = $derived.by(() => {
		const drag = session.drag.state;
		return drag !== null && drag.track === track && drag.index === index ? drag.kind : null;
	});
	const isDragging = $derived(
		dragging ||
			(session.drag.state !== null &&
				session.drag.state.track === track &&
				session.drag.state.index === index)
	);
	const invalidDrop = $derived(
		isDragging && session.drag.state !== null && session.drag.state.invalidTarget
	);
	const invalidNotice = $derived(
		invalidDrop
			? block.kind === 'midi'
				? 'Cannot place MIDI block on audio track'
				: 'Cannot place audio block on MIDI track'
			: undefined
	);
	const rate = $derived(
		editing === 'stretch' ? (block.lengthBeats * block.rate) / Math.max(lengthBeats, 0.001) : block.rate
	);
	const offsetSeconds = $derived(
		editing === 'trim'
			? block.offsetSeconds + ((startBeats - block.startBeats) * 60 * block.rate) / bpm
			: block.offsetSeconds
	);
	const stretched = $derived(block.kind === 'audio' && Math.abs(rate - 1) > 0.001);
	const visible = $derived(
		wave === null
			? null
			: waveWindow(offsetSeconds, lengthBeats, bpm, rate, wave.seconds, wave.peaks.length)
	);
	const wavePoints = $derived(
		wave === null || visible === null
			? ''
			: waveformPoints(wave.peaks, WAVE_HEIGHT, visible, width)
	);
	const fadeInBeats = $derived(
		editing === 'fade-in' && session.drag.state !== null
			? session.drag.state.fadeInBeats
			: (block.fadeInBeats ?? 0)
	);
	const fadeOutBeats = $derived(
		editing === 'fade-out' && session.drag.state !== null
			? session.drag.state.fadeOutBeats
			: (block.fadeOutBeats ?? 0)
	);
	const fadeInPx = $derived(Math.min(width, Math.max(0, beatsToPx(fadeInBeats, session.zoom))));
	const fadeOutPx = $derived(
		Math.min(Math.max(0, width - fadeInPx), Math.max(0, beatsToPx(fadeOutBeats, session.zoom)))
	);
	const hasFades = $derived(block.kind === 'audio' && (fadeInPx > 0.5 || fadeOutPx > 0.5));

	$effect(() => {
		const source = block.kind === 'audio' ? block.source : '';
		if (source === '') {
			wave = null;
			return;
		}
		let live = true;
		void loadWaveform(source)
			.then((value) => {
				if (live) wave = value;
			})
			.catch(() => {
				if (live) wave = null;
			});
		return () => {
			live = false;
		};
	});

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		const now = performance.now();
		if (event.detail >= 2 || now - lastDownAt < 320) {
			lastDownAt = 0;
			event.stopPropagation();
			event.preventDefault();
			session.drag.cancel();
			onDoubleClick();
			return;
		}
		lastDownAt = now;
		event.stopPropagation();
		const element = event.currentTarget;
		if (!(element instanceof HTMLElement)) return;
		const grab = pxToBeats(event.clientX - element.getBoundingClientRect().left, session.zoom);
		element.setPointerCapture(event.pointerId);
		session.dispatch({ selectBlock: { track, block: index } });
		selection.clear();
		const open = session.roll;
		if (open === null || open.track !== track || open.block !== index) {
			session.openRoll(track, index);
		} else {
			detail.showClip();
		}
		session.drag.beginMove(track, index, block, grab);
	}

	function onFadeInPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.stopPropagation();
		const element = event.currentTarget;
		if (element instanceof HTMLElement) element.setPointerCapture(event.pointerId);
		session.dispatch({ selectBlock: { track, block: index } });
		selection.clear();
		session.drag.beginFadeIn(track, index, block);
	}

	function onFadeOutPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.stopPropagation();
		const element = event.currentTarget;
		if (element instanceof HTMLElement) element.setPointerCapture(event.pointerId);
		session.dispatch({ selectBlock: { track, block: index } });
		selection.clear();
		session.drag.beginFadeOut(track, index, block);
	}

	function resetFadeIn(event: MouseEvent): void {
		event.stopPropagation();
		session.dispatch('checkpoint');
		session.dispatch({
			setBlockFades: { track, block: index, fadeInBeats: 0, fadeOutBeats }
		});
	}

	function resetFadeOut(event: MouseEvent): void {
		event.stopPropagation();
		session.dispatch('checkpoint');
		session.dispatch({
			setBlockFades: { track, block: index, fadeInBeats, fadeOutBeats: 0 }
		});
	}

	function onResizePointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.stopPropagation();
		const element = event.currentTarget;
		if (element instanceof HTMLElement) element.setPointerCapture(event.pointerId);
		session.dispatch({ selectBlock: { track, block: index } });
		selection.clear();
		session.drag.beginResize(track, index, block, event.shiftKey);
	}

	function onTrimPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.stopPropagation();
		const element = event.currentTarget;
		if (element instanceof HTMLElement) element.setPointerCapture(event.pointerId);
		session.dispatch({ selectBlock: { track, block: index } });
		selection.clear();
		session.drag.beginTrim(track, index, block);
	}

	function onDoubleClick(): void {
		session.selectTrack(track);
		session.dispatch({ selectBlock: { track, block: index } });
		session.openRoll(track, index);
	}

	function onContextMenu(event: MouseEvent): void {
		event.preventDefault();
		event.stopPropagation();
		const element = event.currentTarget;
		if (!(element instanceof HTMLElement)) return;
		const at = beatUnder(block.startBeats, event.clientX - element.getBoundingClientRect().left, session.zoom);
		session.dispatch({ selectBlock: { track, block: index } });
		menu.open(event.clientX, event.clientY, blockMenu(session, track, index, block, at, gridStep(session.zoom), event.altKey));
	}
</script>

<div
	class="block"
	class:selected={block.selected}
	class:dragging
	class:invalid-drop={invalidDrop}
	role="listitem"
	title={invalidNotice}
	aria-label={invalidNotice}
	data-block={index}
	data-block-track={track}
	style="left: {left}px; width: {width}px; --fill: {fillColor}"
	onpointerdown={onPointerDown}
	ondblclick={onDoubleClick}
	oncontextmenu={onContextMenu}
>
	{#if block.kind === 'audio' && wave !== null && visible !== null && wavePoints !== ''}
		<svg
			class="wave"
			viewBox="{visible.x} 0 {visible.width} {WAVE_HEIGHT}"
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			<polygon points={wavePoints} fill={fillColor} stroke={fillColor} />
		</svg>
	{/if}

	{#if hasFades}
		{@const outStart = width - fadeOutPx}
		{@const inMid = fadeInPx * 0.5}
		{@const outMid = outStart + fadeOutPx * 0.5}
		<svg
			class="fade-overlay"
			viewBox="0 0 {width} 40"
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			{#if fadeInPx > 0.5}
				<path
					class="fade-shade"
					d="M 0 0 L {fadeInPx.toFixed(1)} 0 C {inMid.toFixed(1)} 0, {inMid.toFixed(1)} 40, 0 40 Z"
				/>
				<path
					class="fade-curve"
					d="M 0 40 C {inMid.toFixed(1)} 40, {inMid.toFixed(1)} 0, {fadeInPx.toFixed(1)} 0"
				/>
			{/if}
			{#if fadeOutPx > 0.5}
				<path
					class="fade-shade"
					d="M {outStart.toFixed(1)} 0 L {width.toFixed(1)} 0 L {width.toFixed(1)} 40 C {outMid.toFixed(1)} 40, {outMid.toFixed(1)} 0, {outStart.toFixed(1)} 0 Z"
				/>
				<path
					class="fade-curve"
					d="M {outStart.toFixed(1)} 0 C {outMid.toFixed(1)} 0, {outMid.toFixed(1)} 40, {width.toFixed(1)} 40"
				/>
			{/if}
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
					fill={fillColor}
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

	{#if stretched}
		<span class="rate">&times;{rate.toFixed(2)}</span>
	{/if}

	<button
		type="button"
		class="handle start"
		aria-label="Trim start of {block.name}"
		onpointerdown={onTrimPointerDown}
	></button>

	<button
		type="button"
		class="handle"
		aria-label="Resize {block.name}, shift to stretch"
		onpointerdown={onResizePointerDown}
	></button>

	{#if block.kind === 'audio'}
		<button
			type="button"
			class="fade-handle fade-in"
			class:active={fadeInBeats > 0.001 || editing === 'fade-in'}
			style="left: {Math.min(Math.max(0, fadeInPx - 4), Math.max(0, width - 10))}px"
			title="Drag to adjust fade-in ({fadeInBeats.toFixed(2)} beats), double-click to reset"
			aria-label="Fade in {block.name}"
			onpointerdown={onFadeInPointerDown}
			ondblclick={resetFadeIn}
		></button>
		<button
			type="button"
			class="fade-handle fade-out"
			class:active={fadeOutBeats > 0.001 || editing === 'fade-out'}
			style="right: {Math.min(Math.max(0, fadeOutPx - 4), Math.max(0, width - 10))}px"
			title="Drag to adjust fade-out ({fadeOutBeats.toFixed(2)} beats), double-click to reset"
			aria-label="Fade out {block.name}"
			onpointerdown={onFadeOutPointerDown}
			ondblclick={resetFadeOut}
		></button>
	{/if}
</div>

<style>
	.block {
		position: absolute;
		top: 2px;
		bottom: 2px;
		height: calc(100% - 4px);
		display: flex;
		align-items: flex-start;
		overflow: hidden;
		border-left: 2px solid var(--fill);
		outline: 1px solid transparent;
		outline-offset: -1px;
		background: color-mix(in srgb, var(--fill) 16%, var(--surface));
		font-size: 10px;
		cursor: grab;
		user-select: none;
		z-index: 1;
	}

	.block.selected {
		outline-color: var(--text);
		z-index: 2;
	}

	.block.dragging {
		opacity: 0.7;
		cursor: grabbing;
	}

	.block.invalid-drop {
		opacity: 0.35;
		cursor: not-allowed !important;
		outline: 1px dashed var(--danger, #ff4444);
	}

	.block.invalid-drop .handle {
		cursor: not-allowed !important;
		pointer-events: none;
	}

	.wave {
		position: absolute;
		inset: 14px 0 2px;
		width: 100%;
		height: calc(100% - 16px);
		pointer-events: none;
	}

	.wave polygon {
		fill: var(--fill);
		fill-opacity: 0.85;
		stroke: var(--fill);
		stroke-width: 1px;
		vector-effect: non-scaling-stroke;
	}

	.wave .bar {
		fill: var(--fill);
	}

	.name {
		position: relative;
		padding: 2px 12px 0 6px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text);
		letter-spacing: 0.04em;
		pointer-events: none;
	}

	.rename {
		position: relative;
		width: calc(100% - 8px);
		margin: 1px 0 0 4px;
		padding: 0 4px;
		height: 14px;
		font: inherit;
		color: var(--text);
		background: var(--bg);
		border: 1px solid var(--green);
	}

	.handle {
		position: absolute;
		top: 0;
		right: 0;
		width: 6px;
		height: 100%;
		padding: 0;
		border: 0;
		background: transparent;
		cursor: ew-resize;
		transition: background 0.15s ease;
	}

	.handle.start {
		right: auto;
		left: 0;
	}

	.handle:hover:not(:disabled) {
		filter: none;
		background: color-mix(in srgb, var(--text) 18%, transparent);
	}

	.rate {
		position: absolute;
		top: 2px;
		right: 9px;
		font-size: 9px;
		letter-spacing: 0.04em;
		font-variant-numeric: tabular-nums;
		color: var(--text-subtle);
	}

	.fade-overlay {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		pointer-events: none;
		z-index: 1;
	}

	.fade-shade {
		fill: rgba(0, 0, 0, 0.42);
	}

	.fade-curve {
		fill: none;
		stroke: var(--text);
		stroke-width: 1.25px;
		vector-effect: non-scaling-stroke;
		opacity: 0.85;
	}

	.fade-handle {
		position: absolute;
		top: 0;
		width: 7px;
		height: 7px;
		padding: 0;
		border: 1px solid var(--text);
		background: var(--bg);
		cursor: ew-resize;
		opacity: 0;
		z-index: 3;
	}

	.block:hover .fade-handle,
	.block.selected .fade-handle,
	.fade-handle.active {
		opacity: 1;
	}

	.fade-handle:hover,
	.fade-handle.active {
		background: var(--text);
	}
</style>
