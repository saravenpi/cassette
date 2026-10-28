<script lang="ts">
	/**
	 * The bar and beat ruler. Double-click to seek the transport; shift-drag to
	 * draw the loop region, and double-click the region to loop the whole song.
	 */

	import { RULER_HEIGHT, beatsToPx, gridStep, labelStep, pxToBeats, snapBeats } from '$lib/geometry';
	import { session } from '$lib/session.svelte';
	import type { LoopRegion } from '$lib/wire';

	interface Props {
		pxPerBeat: number;
		beatsPerBar: number;
		bars: number;
		width?: number;
	}

	let { pxPerBeat, beatsPerBar, bars, width }: Props = $props();

	interface Mark {
		key: string;
		left: number;
		label: string | null;
		bar: boolean;
	}

	const barPx = $derived(beatsToPx(beatsPerBar, pxPerBeat));
	const step = $derived(labelStep(barPx));
	const beatTicks = $derived(pxPerBeat >= 12);

	const marks = $derived.by((): Mark[] => {
		const out: Mark[] = [];
		for (let bar = 0; bar < bars; bar += 1) {
			const left = bar * barPx;
			out.push({
				key: `bar-${bar}`,
				left,
				label: bar % step === 0 ? String(bar + 1) : null,
				bar: true
			});
			if (!beatTicks) continue;
			for (let beat = 1; beat < beatsPerBar; beat += 1) {
				out.push({ key: `beat-${bar}-${beat}`, left: left + beat * pxPerBeat, label: null, bar: false });
			}
		}
		return out;
	});

	let drawing = $state<LoopRegion | null>(null);

	const region = $derived(drawing ?? session.snapshot?.loopRegion ?? null);

	function beatsAt(event: MouseEvent): number | null {
		const ruler = document.querySelector('.ruler');
		if (!ruler) return null;
		const rect = ruler.getBoundingClientRect();
		const px = Math.max(0, event.clientX - rect.left);
		return snapBeats(Math.max(0, pxToBeats(px, pxPerBeat)), gridStep(pxPerBeat));
	}

	function seekFromPointer(event: MouseEvent): void {
		const beats = beatsAt(event);
		if (beats !== null) session.dispatch({ seek: { beats } });
	}

	function drawFromPointer(anchor: number, event: PointerEvent): void {
		const beats = beatsAt(event);
		if (beats === null) return;
		drawing = { startBeats: Math.min(anchor, beats), endBeats: Math.max(anchor, beats) };
	}

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0 || !event.shiftKey) return;
		const anchor = beatsAt(event);
		if (anchor === null) return;
		const onMove = (e: PointerEvent) => drawFromPointer(anchor, e);
		const onUp = () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
			if (drawing !== null && drawing.endBeats > drawing.startBeats) {
				session.dispatch({ setLoopRegion: { ...drawing } });
			}
			drawing = null;
		};
		drawing = { startBeats: anchor, endBeats: anchor };
		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
	}

	function clearRegion(event: Event): void {
		event.preventDefault();
		event.stopPropagation();
		session.dispatch({ setLoopRegion: { startBeats: 0, endBeats: 0 } });
	}
</script>

<div
	class="ruler"
	role="presentation"
	style="width: {width !== undefined ? `${width}px` : '100%'}; height: {RULER_HEIGHT}px"
	onpointerdown={onPointerDown}
	ondblclick={seekFromPointer}
>
	{#if region !== null}
		<button
			type="button"
			class="loop"
			class:off={!session.snapshot?.looping}
			aria-label="Loop region, beats {region.startBeats + 1} to {region.endBeats + 1}; double-click to loop the whole song"
			style="left: {beatsToPx(region.startBeats, pxPerBeat)}px; width: {beatsToPx(region.endBeats - region.startBeats, pxPerBeat)}px"
			ondblclick={clearRegion}
			onkeydown={(event) => {
				if (event.key === 'Delete' || event.key === 'Backspace') clearRegion(event);
			}}
		></button>
	{/if}
	{#each marks as mark (mark.key)}
		<span class="mark" class:bar={mark.bar} style="left: {mark.left}px">
			{#if mark.label !== null}<span class="label">{mark.label}</span>{/if}
		</span>
	{/each}
</div>

<style>
	.ruler {
		position: relative;
		flex: none;
		background: var(--surface);
		cursor: pointer;
		user-select: none;
	}

	.loop {
		position: absolute;
		top: 0;
		height: 100%;
		padding: 0;
		border: 0;
		border-left: 1px solid var(--green);
		border-right: 1px solid var(--green);
		border-top: 2px solid var(--green);
		background: var(--green-soft);
		cursor: pointer;
	}

	.loop:hover:not(:disabled) {
		filter: none;
	}

	.loop.off {
		opacity: 0.35;
	}

	.mark {
		position: absolute;
		bottom: 0;
		width: 1px;
		height: 5px;
		background: var(--border);
		pointer-events: none;
	}

	.mark.bar {
		height: 12px;
		background: var(--grid-major);
	}

	.label {
		position: absolute;
		top: -18px;
		left: 4px;
		font-size: 10px;
		letter-spacing: 0.04em;
		color: var(--text-subtle);
		font-variant-numeric: tabular-nums;
	}
</style>
