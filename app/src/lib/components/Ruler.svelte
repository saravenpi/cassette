<script lang="ts">
	/**
	 * The bar and beat ruler. Click or drag to seek the transport.
	 */

	import { RULER_HEIGHT, beatsToPx, labelStep, pxToBeats, snapBeats } from '$lib/geometry';
	import { session } from '$lib/session.svelte';

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

	function seekFromPointer(event: PointerEvent): void {
		const ruler = document.querySelector('.ruler');
		if (!ruler) return;
		const rect = ruler.getBoundingClientRect();
		const px = Math.max(0, event.clientX - rect.left);
		const beats = Math.max(0, pxToBeats(px, pxPerBeat));
		session.dispatch({ seek: { beats: snapBeats(beats) } });
	}

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		seekFromPointer(event);
		const onMove = (e: PointerEvent) => seekFromPointer(e);
		const onUp = () => {
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
		};
		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
	}
</script>

<div
	class="ruler"
	role="presentation"
	style="width: {width !== undefined ? `${width}px` : '100%'}; height: {RULER_HEIGHT}px"
	onpointerdown={onPointerDown}
>
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
