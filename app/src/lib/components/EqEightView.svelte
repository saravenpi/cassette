<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import {
		computeCompositeCurve,
		EQ_FILTER_TYPES,
		formatFreq,
		FREQ_GRID,
		freqToX,
		GAIN_GRID,
		gainToY,
		isBandActive,
		unpackBands,
		xToFreq,
		yToGain,
		type EqBand
	} from '$lib/eqEight';
	import { specFor } from '$lib/chain';
	import { session } from '$lib/session.svelte';
	import { MASTER_TRACK, type EffectView } from '$lib/wire';
	import Knob from './Knob.svelte';

	const SPECTRUM_BINS = 36;

	interface Props {
		effect: EffectView;
		index: number;
		at: number;
	}

	let { effect, index, at }: Props = $props();

	let displayWidth = $state(380);
	let displayHeight = $state(130);
	const WIDTH = $derived(Math.max(120, displayWidth));
	const HEIGHT = $derived(Math.max(80, displayHeight));

	let selectedBand = $state(2);
	let dragging = $state<{ band: number; startX: number; startY: number; startQ: number } | null>(null);
	let svgElem = $state<SVGSVGElement | null>(null);

	const catalog = $derived(session.snapshot?.effectCatalog ?? []);
	const spec = $derived(specFor(catalog, effect.kind));
	const bands = $derived(unpackBands(effect.params));
	const curve = $derived(computeCompositeCurve(bands, WIDTH, HEIGHT));
	const currentBand = $derived(bands[selectedBand] ?? bands[0]);

	const freqSpec = $derived(spec?.params[selectedBand * 4]);
	const gainSpec = $derived(spec?.params[selectedBand * 4 + 1]);
	const qSpec = $derived(spec?.params[selectedBand * 4 + 2]);

	let spectrum = $state<number[]>(new Array(SPECTRUM_BINS).fill(0));
	let animId: number | null = null;

	const currentLevel = $derived.by(() => {
		if (index === MASTER_TRACK) {
			return session.snapshot?.masterLevel ?? 0;
		}
		return session.snapshot?.tracks[index]?.level ?? 0;
	});

	const isPlaying = $derived(session.snapshot?.playing ?? false);

	function animateSpectrum(): void {
		const level = currentLevel;
		const playing = isPlaying && level > 0.002;
		const now = performance.now() * 0.003;

		const next = spectrum.slice();
		for (let i = 0; i < SPECTRUM_BINS; i++) {
			if (playing) {
				const frac = i / (SPECTRUM_BINS - 1);
				const f = 30 * Math.pow(18000 / 30, frac);
				const sub = Math.exp(-Math.pow((Math.log(f) - Math.log(80)) / 1.0, 2));
				const lowmid = 0.8 * Math.exp(-Math.pow((Math.log(f) - Math.log(300)) / 1.2, 2));
				const mid = 0.6 * Math.exp(-Math.pow((Math.log(f) - Math.log(1500)) / 1.5, 2));
				const air = 0.4 * Math.exp(-Math.pow((Math.log(f) - Math.log(8000)) / 1.8, 2));
				const waves =
					Math.sin(now * 2.1 + i * 0.4) * 0.2 +
					Math.sin(now * 4.7 + i * 0.8) * 0.15 +
					Math.sin(now * 1.3 - i * 0.3) * 0.25;
				let target = level * (sub + lowmid + mid + air + waves) * 1.4;
				let eqOffsetDb = 0;
				for (const b of bands) {
					eqOffsetDb += (b.gain * Math.exp(-Math.pow(Math.log(f / b.freq) / (1.0 / b.q), 2))) * 0.05;
				}
				target = Math.max(0, target * Math.pow(10, eqOffsetDb / 20));
				next[i] += (target - next[i]) * 0.25;
			} else {
				next[i] *= 0.84;
			}
		}
		spectrum = next;
		animId = requestAnimationFrame(animateSpectrum);
	}

	onMount(() => {
		animId = requestAnimationFrame(animateSpectrum);
	});

	onDestroy(() => {
		if (animId !== null) cancelAnimationFrame(animId);
		stopDrag();
	});

	const spectrumPath = $derived.by(() => {
		if (spectrum.every((v) => v < 0.001)) return '';
		const pts: [number, number][] = [];
		for (let i = 0; i < SPECTRUM_BINS; i++) {
			const frac = i / (SPECTRUM_BINS - 1);
			const f = 30 * Math.pow(18000 / 30, frac);
			const x = freqToX(f, WIDTH);
			const amp = Math.min(1.0, spectrum[i]);
			const y = HEIGHT - amp * (HEIGHT - 10);
			pts.push([x, y]);
		}
		let d = `M ${pts[0][0].toFixed(1)} ${HEIGHT}`;
		d += ` L ${pts[0][0].toFixed(1)} ${pts[0][1].toFixed(1)}`;
		for (let i = 1; i < pts.length; i++) {
			d += ` L ${pts[i][0].toFixed(1)} ${pts[i][1].toFixed(1)}`;
		}
		d += ` L ${pts[pts.length - 1][0].toFixed(1)} ${HEIGHT} Z`;
		return d;
	});

	function nodePos(band: EqBand): { x: number; y: number } {
		const x = freqToX(band.freq, WIDTH);
		const y = band.type === 0 || band.type === 4 || band.type === 5 ? gainToY(0, HEIGHT) : gainToY(band.gain, HEIGHT);
		return { x, y };
	}

	function onNodePointerDown(event: PointerEvent, b: number): void {
		selectedBand = b;
		session.dispatch('checkpoint');
		event.preventDefault();
		event.stopPropagation();
		const startQ = bands[b]?.q ?? 1.0;
		dragging = { band: b, startX: event.clientX, startY: event.clientY, startQ };
		window.addEventListener('pointermove', onPointerMove);
		window.addEventListener('pointerup', onPointerUp);
		window.addEventListener('pointercancel', onPointerUp);
	}

	function onPointerMove(event: PointerEvent): void {
		if (dragging === null || svgElem === null) return;
		const rect = svgElem.getBoundingClientRect();
		const mouseX = Math.max(0, Math.min(WIDTH, ((event.clientX - rect.left) / rect.width) * WIDTH));
		const mouseY = Math.max(0, Math.min(HEIGHT, ((event.clientY - rect.top) / rect.height) * HEIGHT));
		const b = dragging.band;
		const band = bands[b];
		if (!band) return;

		const newFreq = Math.round(xToFreq(mouseX, WIDTH));
		session.queueEffectParam(index, at, b * 4, newFreq);

		if (band.type === 0 || band.type === 4) {
			const dy = dragging.startY - event.clientY;
			const newQ = Math.max(0.1, Math.min(10.0, +(dragging.startQ * (1 + dy * 0.01)).toFixed(2)));
			session.queueEffectParam(index, at, b * 4 + 2, newQ);
		} else if (band.type !== 5) {
			const newGain = +yToGain(mouseY, HEIGHT).toFixed(1);
			session.queueEffectParam(index, at, b * 4 + 1, newGain);
		}
	}

	function onPointerUp(): void {
		stopDrag();
	}

	function stopDrag(): void {
		dragging = null;
		window.removeEventListener('pointermove', onPointerMove);
		window.removeEventListener('pointerup', onPointerUp);
		window.removeEventListener('pointercancel', onPointerUp);
	}

	function onNodeWheel(event: WheelEvent, b: number): void {
		event.preventDefault();
		event.stopPropagation();
		const band = bands[b];
		if (!band) return;
		const factor = event.deltaY > 0 ? 0.9 : 1.1;
		const nextQ = Math.max(0.1, Math.min(10.0, +(band.q * factor).toFixed(2)));
		session.queueEffectParam(index, at, b * 4 + 2, nextQ);
	}

	function setBandType(t: number): void {
		session.dispatch('checkpoint');
		session.queueEffectParam(index, at, selectedBand * 4 + 3, t);
	}
</script>

<div class="eq-eight" role="region" aria-label="EQ Eight visual equalizer">
	<div class="display" bind:clientWidth={displayWidth} bind:clientHeight={displayHeight}>
		<svg
			bind:this={svgElem}
			viewBox="0 0 {WIDTH} {HEIGHT}"
			class="display-svg"
		>
			<defs>
				<linearGradient id="curveGradient" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color="var(--green)" stop-opacity="0.32" />
					<stop offset="100%" stop-color="var(--green)" stop-opacity="0.02" />
				</linearGradient>
				<linearGradient id="spectrumGradient" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color="var(--blue)" stop-opacity="0.22" />
					<stop offset="100%" stop-color="var(--blue)" stop-opacity="0.0" />
				</linearGradient>
			</defs>

			<g class="grid-horizontal">
				{#each GAIN_GRID as gain}
					{@const y = gainToY(gain, HEIGHT)}
					<line
						x1="0"
						y1={y}
						x2={WIDTH}
						y2={y}
						class="grid-line"
						class:zero={gain === 0}
					/>
					<text x={WIDTH - 4} y={y - 2} class="grid-label gain-label">
						{gain > 0 ? `+${gain}` : `${gain}`}
					</text>
				{/each}
			</g>

			<g class="grid-vertical">
				{#each FREQ_GRID as freq}
					{@const x = freqToX(freq, WIDTH)}
					<line x1={x} y1="0" x2={x} y2={HEIGHT} class="grid-line" />
					<text x={x} y={HEIGHT - 4} class="grid-label freq-label">
						{formatFreq(freq)}
					</text>
				{/each}
			</g>

			{#if spectrumPath}
				<path d={spectrumPath} fill="url(#spectrumGradient)" />
			{/if}

			{#if curve.area}
				<path d={curve.area} fill="url(#curveGradient)" />
			{/if}

			{#if curve.path}
				<path d={curve.path} class="eq-curve" />
			{/if}

			<g class="nodes">
				{#each bands as band, b (b)}
					{@const pos = nodePos(band)}
					<g
						class="node"
						class:selected={selectedBand === b}
						class:active={isBandActive(band)}
						transform="translate({pos.x.toFixed(1)}, {pos.y.toFixed(1)})"
						onpointerdown={(e) => onNodePointerDown(e, b)}
						onwheel={(e) => onNodeWheel(e, b)}
						role="button"
						tabindex="0"
						aria-label="Band {b + 1}"
					>
						<circle r="9" class="node-bg" />
						<circle r="8" class="node-ring" />
						<text dy="3.5" class="node-text">{b + 1}</text>
					</g>
				{/each}
			</g>
		</svg>
	</div>

	<div class="controls-bar">
		<div class="band-selector" role="tablist" aria-label="Select band">
			{#each [1, 2, 3, 4, 5, 6, 7, 8] as num, b}
				<button
					type="button"
					role="tab"
					class="band-toggle"
					class:selected={selectedBand === b}
					class:active={isBandActive(bands[b])}
					onclick={() => (selectedBand = b)}
					aria-selected={selectedBand === b}
				>
					<span class="active-dot"></span>
					<span class="band-num">{num}</span>
				</button>
			{/each}
		</div>

		<div class="band-strip">
			<div class="types-row">
				{#each EQ_FILTER_TYPES as typeName, t}
					<button
						type="button"
						class="type-pill"
						class:selected={currentBand.type === t}
						onclick={() => setBandType(t)}
					>
						{typeName}
					</button>
				{/each}
			</div>

			<div class="dials-row">
				{#if freqSpec}
					<Knob
						spec={freqSpec}
						value={currentBand.freq}
						label="Band {selectedBand + 1} Freq"
						onbegin={() => session.dispatch('checkpoint')}
						oninput={(val) => session.queueEffectParam(index, at, selectedBand * 4, val)}
					/>
				{/if}

				{#if gainSpec}
					<Knob
						spec={gainSpec}
						value={currentBand.gain}
						disabled={currentBand.type === 0 || currentBand.type === 4 || currentBand.type === 5}
						label="Band {selectedBand + 1} Gain"
						onbegin={() => session.dispatch('checkpoint')}
						oninput={(val) => session.queueEffectParam(index, at, selectedBand * 4 + 1, val)}
					/>
				{/if}

				{#if qSpec}
					<Knob
						spec={qSpec}
						value={currentBand.q}
						label="Band {selectedBand + 1} Q"
						onbegin={() => session.dispatch('checkpoint')}
						oninput={(val) => session.queueEffectParam(index, at, selectedBand * 4 + 2, val)}
					/>
				{/if}
			</div>
		</div>
	</div>
</div>

<style>
	.eq-eight {
		display: flex;
		gap: 8px;
		width: 100%;
		min-width: 560px;
		height: 100%;
		user-select: none;
	}

	.display {
		position: relative;
		flex: 1;
		min-width: 300px;
		min-height: 100px;
		background: #090909;
		border: 1px solid var(--border);
		overflow: hidden;
	}

	.display-svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		display: block;
	}

	.grid-line {
		stroke: var(--grid-minor);
		stroke-width: 1;
		shape-rendering: crispEdges;
	}

	.grid-line.zero {
		stroke: var(--grid-major);
		stroke-dasharray: 2 2;
	}

	.grid-label {
		fill: var(--dim);
		font-size: 8px;
		letter-spacing: 0.04em;
	}

	.gain-label {
		text-anchor: end;
	}

	.freq-label {
		text-anchor: middle;
	}

	.eq-curve {
		fill: none;
		stroke: var(--green);
		stroke-width: 1.75;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.node {
		cursor: grab;
	}

	.node:active {
		cursor: grabbing;
	}

	.node-bg {
		fill: var(--surface);
	}

	.node-ring {
		fill: transparent;
		stroke: var(--border);
		stroke-width: 1.5;
		transition: stroke 0.1s ease;
	}

	.node.active .node-ring {
		stroke: var(--green);
	}

	.node.selected .node-bg {
		fill: var(--green);
	}

	.node.selected .node-ring {
		stroke: var(--text);
		stroke-width: 2;
	}

	.node-text {
		font-size: 9px;
		fill: var(--text-subtle);
		text-anchor: middle;
		dominant-baseline: middle;
	}

	.node.active .node-text {
		fill: var(--text);
	}

	.node.selected .node-text {
		fill: var(--on-green);
	}

	.controls-bar {
		flex: none;
		width: 232px;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		gap: 6px;
		min-height: 0;
		background: var(--surface);
		border: 1px solid var(--border-subtle);
		padding: 6px;
	}

	.band-selector {
		display: flex;
		gap: 3px;
	}

	.band-toggle {
		flex: 1;
		height: 22px;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 4px;
		background: transparent;
		border: 1px solid var(--border);
		color: var(--dim);
		font-size: 10px;
		padding: 0;
		cursor: pointer;
	}

	.band-toggle:hover {
		background: var(--surface-hover);
		color: var(--text);
	}

	.band-toggle.selected {
		background: var(--surface-hover);
		border-color: var(--green);
		color: var(--text);
	}

	.active-dot {
		width: 4px;
		height: 4px;
		background: transparent;
		border-radius: 50% !important;
	}

	.band-toggle.active .active-dot {
		background: var(--green);
	}

	.band-strip {
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 6px;
	}

	.types-row {
		display: flex;
		flex-wrap: wrap;
		gap: 2px;
	}

	.type-pill {
		height: 20px;
		padding: 0 6px;
		font-size: 9px;
		background: transparent;
		border: 1px solid var(--border);
		color: var(--dim);
		cursor: pointer;
	}

	.type-pill:hover {
		background: var(--surface-hover);
		color: var(--text);
	}

	.type-pill.selected {
		background: var(--green);
		border-color: var(--green);
		color: var(--on-green);
	}

	.dials-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 4px;
	}
</style>
