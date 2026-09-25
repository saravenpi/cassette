<script lang="ts">
	/**
	 * The bar and beat ruler. It is a readout, not a control: the transport cannot
	 * be seeked, so nothing here answers a click.
	 */

	import { RULER_HEIGHT, beatsToPx, labelStep } from '$lib/geometry';

	interface Props {
		pxPerBeat: number;
		beatsPerBar: number;
		bars: number;
	}

	let { pxPerBeat, beatsPerBar, bars }: Props = $props();

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
</script>

<div class="ruler" role="presentation" style="height: {RULER_HEIGHT}px">
	{#each marks as mark (mark.key)}
		<span class="mark" class:bar={mark.bar} style="left: {mark.left}px">
			{#if mark.label !== null}<span class="label">{mark.label}</span>{/if}
		</span>
	{/each}
</div>

<style>
	.ruler {
		position: relative;
		flex: 1;
		background: var(--panel);
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
