<script lang="ts">
	/**
	 * The bar and beat ruler. Click or drag to seek the transport; shift-drag to
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

	function beatsAt(event: PointerEvent): number | null {
		const ruler = document.querySelector('.ruler');
		if (!ruler) return null;
		const rect = ruler.getBoundingClientRect();
		const px = Math.max(0, event.clientX - rect.left);
		return snapBeats(Math.max(0, pxToBeats(px, pxPerBeat)), gridStep(pxPerBeat));
	}

	function seekFromPointer(event: PointerEvent): void {
		const beats = beatsAt(event);
		if (beats !== null) session.dispatch({ seek: { beats } });
	}

	function drawFromPointer(anchor: number, event: PointerEvent): void {
		const beats = beatsAt(event);
		if (beats === null) return;
		drawing = { startBeats: Math.min(anchor, beats), endBeats: Math.max(anchor, beats) };
	}

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		const anchor = event.shiftKey ? beatsAt(event) : null;
		const onMove =
			anchor === null
				? (e: PointerEvent) => seekFromPointer(e)
				: (e: PointerEvent) => drawFromPointer(anchor, e);
		const onUp = () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
			if (drawing !== null && drawing.endBeats > drawing.startBeats) {
				session.dispatch({ setLoopRegion: { ...drawing } });
			}
			drawing = null;
		};
		if (anchor === null) seekFromPointer(event);
		else drawing = { startBeats: anchor, endBeats: anchor };
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
		background: var(--panel);
		cursor: pointer;
		user-select: none;
	}

	.loop {
		position: absolute;
		top: 0;
		height: 6px;
		padding: 0;
		border: none;
		border-radius: 0 0 3px 3px;
		background: var(--accent);
		opacity: 0.8;
		cursor: pointer;
	}

	.loop.off {
		opacity: 0.3;
	}

	.mark {
		position: absolute;
		bottom: 0;
		width: 1px;
		height: 6px;
		background: var(--line);
	}

	.mark.bar {
		height: 12px;
		background: var(--dim);
	}

	.label {
		position: absolute;
		top: -22px;
		left: 3px;
		font-size: 10px;
		color: var(--dim);
		font-variant-numeric: tabular-nums;
	}
</style>
