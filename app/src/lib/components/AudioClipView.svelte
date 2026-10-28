<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { loadSpectrum, loadWaveform } from '$lib/api';
	import { formatFreq, FREQ_GRID, freqToX } from '$lib/eqEight';
	import { waveWindow, waveformPoints } from '$lib/geometry';
	import { trackColor } from '$lib/palette';
	import { session } from '$lib/session.svelte';
	import type { BlockView, ClipSpectrum, TrackView, Waveform } from '$lib/wire';

	const WAVE_HEIGHT = 120;
	const BINS = 36;
	const DB_GRID = [-12, -24, -36, -48];

	interface Props {
		track: number;
		blockIndex: number;
		block: BlockView;
		trackView: TrackView;
	}

	let { track, blockIndex, block, trackView }: Props = $props();

	let wave = $state<Waveform | null>(null);
	let specData = $state<ClipSpectrum | null>(null);
	let specWidth = $state(360);
	let specHeight = $state(160);
	let waveWidth = $state(360);
	let liveBins = $state<number[]>(new Array(BINS).fill(0));
	let peakBins = $state<number[]>(new Array(BINS).fill(0));
	let animId: number | null = null;

	const fillColor = $derived(trackColor(trackView.colorIndex));
	const bpm = $derived(session.snapshot?.bpm ?? 120);
	const isPlaying = $derived(session.snapshot?.playing ?? false);
	const playheadBeats = $derived(session.snapshot?.transportPositionBeats ?? 0);
	const fadeInBeats = $derived(block.fadeInBeats ?? 0);
	const fadeOutBeats = $derived(block.fadeOutBeats ?? 0);

	const playheadInClip = $derived(
		playheadBeats >= block.startBeats && playheadBeats <= block.startBeats + block.lengthBeats
	);
	const clipProgress = $derived(
		block.lengthBeats > 0
			? Math.max(0, Math.min(1, (playheadBeats - block.startBeats) / block.lengthBeats))
			: 0
	);

	const visible = $derived(
		wave === null
			? null
			: waveWindow(
					block.offsetSeconds,
					block.lengthBeats,
					bpm,
					block.rate,
					wave.seconds,
					wave.peaks.length
				)
	);

	const wavePoints = $derived(
		wave === null || visible === null
			? ''
			: waveformPoints(wave.peaks, WAVE_HEIGHT, visible, Math.max(120, waveWidth))
	);

	const fileName = $derived.by(() => {
		if (!block.source) return 'No source';
		const parts = block.source.split(/[/\\]/);
		return parts[parts.length - 1] ?? block.source;
	});

	$effect(() => {
		const source = block.source;
		if (!source) {
			wave = null;
			specData = null;
			return;
		}
		let active = true;
		void loadWaveform(source)
			.then((w) => {
				if (active) wave = w;
			})
			.catch(() => {
				if (active) wave = null;
			});
		void loadSpectrum(source)
			.then((s) => {
				if (active) {
					specData = s;
					liveBins = s.overall.slice(0, BINS);
					peakBins = s.overall.slice(0, BINS);
				}
			})
			.catch(() => {
				if (active) specData = null;
			});
		return () => {
			active = false;
		};
	});

	function updateSpectrum(): void {
		const spec = specData;
		if (spec !== null) {
			const next = liveBins.slice();
			const peaks = peakBins.slice();
			if (isPlaying && playheadInClip && spec.slices.length > 0) {
				const clipSec = (block.lengthBeats * 60 * block.rate) / Math.max(20, bpm);
				const currentSec = block.offsetSeconds + clipProgress * clipSec;
				const totalSec = Math.max(0.01, spec.durationSeconds);
				const sliceFrac = Math.max(0, Math.min(0.999, currentSec / totalSec));
				const sliceIdx = Math.floor(sliceFrac * spec.slices.length);
				const slice = spec.slices[sliceIdx] ?? spec.overall;
				let fadeGain = 1;
				const elapsedBeats = playheadBeats - block.startBeats;
				if (fadeInBeats > 0 && elapsedBeats < fadeInBeats) {
					const t = Math.max(0, Math.min(1, elapsedBeats / fadeInBeats));
					fadeGain *= t * t * (3 - 2 * t);
				}
				if (fadeOutBeats > 0 && elapsedBeats > block.lengthBeats - fadeOutBeats) {
					const rem = Math.max(0, block.lengthBeats - elapsedBeats);
					const t = Math.max(0, Math.min(1, rem / fadeOutBeats));
					fadeGain *= t * t * (3 - 2 * t);
				}
				for (let i = 0; i < BINS; i++) {
					const target = (slice[i] ?? 0) * fadeGain;
					next[i] += (target - next[i]) * 0.35;
					peaks[i] = Math.max(peaks[i] * 0.985, next[i]);
				}
			} else {
				for (let i = 0; i < BINS; i++) {
					const target = spec.overall[i] ?? 0;
					next[i] += (target - next[i]) * 0.2;
					peaks[i] = Math.max(peaks[i] * 0.96, next[i]);
				}
			}
			liveBins = next;
			peakBins = peaks;
		}
		animId = requestAnimationFrame(updateSpectrum);
	}

	onMount(() => {
		animId = requestAnimationFrame(updateSpectrum);
	});

	onDestroy(() => {
		if (animId !== null) cancelAnimationFrame(animId);
	});

	const W = $derived(Math.max(160, specWidth));
	const H = $derived(Math.max(90, specHeight));

	const spectrumPaths = $derived.by(() => {
		const pts: [number, number][] = [];
		const peakPts: [number, number][] = [];
		for (let i = 0; i < BINS; i++) {
			const frac = i / (BINS - 1);
			const f = 30 * Math.pow(18000 / 30, frac);
			const x = freqToX(f, W);
			const amp = Math.max(0, Math.min(1, liveBins[i] ?? 0));
			const pk = Math.max(0, Math.min(1, peakBins[i] ?? 0));
			const y = H - 16 - amp * (H - 26);
			const py = H - 16 - pk * (H - 26);
			pts.push([x, y]);
			peakPts.push([x, py]);
		}
		if (pts.length === 0) return { area: '', line: '', peak: '' };
		let line = `M ${pts[0][0].toFixed(1)} ${pts[0][1].toFixed(1)}`;
		let peak = `M ${peakPts[0][0].toFixed(1)} ${peakPts[0][1].toFixed(1)}`;
		for (let i = 1; i < pts.length; i++) {
			line += ` L ${pts[i][0].toFixed(1)} ${pts[i][1].toFixed(1)}`;
			peak += ` L ${peakPts[i][0].toFixed(1)} ${peakPts[i][1].toFixed(1)}`;
		}
		const area = `${line} L ${pts[pts.length - 1][0].toFixed(1)} ${H - 16} L ${pts[0][0].toFixed(1)} ${H - 16} Z`;
		return { area, line, peak };
	});

	function setFadeIn(event: Event): void {
		const val = Number((event.currentTarget as HTMLInputElement).value);
		if (!Number.isFinite(val)) return;
		session.dispatch({
			setBlockFades: {
				track,
				block: blockIndex,
				fadeInBeats: Math.max(0, Math.min(block.lengthBeats - fadeOutBeats, val)),
				fadeOutBeats
			}
		});
	}

	function setFadeOut(event: Event): void {
		const val = Number((event.currentTarget as HTMLInputElement).value);
		if (!Number.isFinite(val)) return;
		session.dispatch({
			setBlockFades: {
				track,
				block: blockIndex,
				fadeInBeats,
				fadeOutBeats: Math.max(0, Math.min(block.lengthBeats - fadeInBeats, val))
			}
		});
	}
</script>

<div class="audio-clip-view" style="--clip-color: {fillColor}">
	<aside class="clip-sidebar">
		<div class="clip-meta">
			<span class="clip-title" title={block.name}>{block.name}</span>
			<span class="clip-file" title={block.source}>{fileName}</span>
		</div>

		<div class="clip-stats">
			<div class="stat-row">
				<span class="stat-label">START</span>
				<span class="stat-val">{block.startBeats.toFixed(2)} b</span>
			</div>
			<div class="stat-row">
				<span class="stat-label">LENGTH</span>
				<span class="stat-val">{block.lengthBeats.toFixed(2)} b</span>
			</div>
			<div class="stat-row">
				<span class="stat-label">RATE</span>
				<span class="stat-val">{block.rate.toFixed(2)}x</span>
			</div>
			<div class="stat-row">
				<span class="stat-label">OFFSET</span>
				<span class="stat-val">{block.offsetSeconds.toFixed(2)}s</span>
			</div>
		</div>

		<div class="fade-controls">
			<label class="fade-row">
				<span class="fade-label">FADE IN</span>
				<input
					type="range"
					min="0"
					max={Math.max(0.25, block.lengthBeats - fadeOutBeats)}
					step="0.05"
					value={fadeInBeats}
					onpointerdown={() => session.dispatch('checkpoint')}
					oninput={setFadeIn}
				/>
				<span class="fade-val">{fadeInBeats.toFixed(2)}b</span>
			</label>
			<label class="fade-row">
				<span class="fade-label">FADE OUT</span>
				<input
					type="range"
					min="0"
					max={Math.max(0.25, block.lengthBeats - fadeInBeats)}
					step="0.05"
					value={fadeOutBeats}
					onpointerdown={() => session.dispatch('checkpoint')}
					oninput={setFadeOut}
				/>
				<span class="fade-val">{fadeOutBeats.toFixed(2)}b</span>
			</label>
		</div>
	</aside>

	<div class="wave-pane" bind:clientWidth={waveWidth}>
		<div class="pane-label">WAVEFORM &amp; FADES</div>
		{#if wave !== null && visible !== null && wavePoints !== ''}
			{@const w = Math.max(120, waveWidth)}
			{@const inW = (fadeInBeats / Math.max(0.001, block.lengthBeats)) * w}
			{@const outW = (fadeOutBeats / Math.max(0.001, block.lengthBeats)) * w}
			{@const outStart = w - outW}
			<svg
				class="clip-wave-svg"
				viewBox="{visible.x} 0 {visible.width} {WAVE_HEIGHT}"
				preserveAspectRatio="none"
			>
				<polygon points={wavePoints} fill={fillColor} stroke={fillColor} />
			</svg>
			<svg class="clip-fade-svg" viewBox="0 0 {w} 100" preserveAspectRatio="none">
				{#if inW > 0.5}
					<path
						class="fade-shade"
						d="M 0 0 L {inW.toFixed(1)} 0 C {(inW * 0.5).toFixed(1)} 0, {(inW * 0.5).toFixed(1)} 100, 0 100 Z"
					/>
					<path
						class="fade-line"
						d="M 0 100 C {(inW * 0.5).toFixed(1)} 100, {(inW * 0.5).toFixed(1)} 0, {inW.toFixed(1)} 0"
					/>
				{/if}
				{#if outW > 0.5}
					<path
						class="fade-shade"
						d="M {outStart.toFixed(1)} 0 L {w.toFixed(1)} 0 L {w.toFixed(1)} 100 C {(outStart + outW * 0.5).toFixed(1)} 100, {(outStart + outW * 0.5).toFixed(1)} 0, {outStart.toFixed(1)} 0 Z"
					/>
					<path
						class="fade-line"
						d="M {outStart.toFixed(1)} 0 C {(outStart + outW * 0.5).toFixed(1)} 0, {(outStart + outW * 0.5).toFixed(1)} 100, {w.toFixed(1)} 100"
					/>
				{/if}
				{#if playheadInClip}
					<line
						x1={clipProgress * w}
						y1="0"
						x2={clipProgress * w}
						y2="100"
						class="playhead-line"
					/>
				{/if}
			</svg>
		{:else}
			<div class="empty-msg">Loading waveform...</div>
		{/if}
	</div>

	<div class="spectrum-pane" bind:clientWidth={specWidth} bind:clientHeight={specHeight}>
		<div class="pane-label">
			<span>AUDIO SPECTRUM</span>
			<span class="pane-mode">{isPlaying && playheadInClip ? 'LIVE' : 'CLIP'}</span>
		</div>
		<svg class="spectrum-svg" viewBox="0 0 {W} {H}">
			<defs>
				<linearGradient id="clipSpecGrad" x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color={fillColor} stop-opacity="0.45" />
					<stop offset="100%" stop-color={fillColor} stop-opacity="0.03" />
				</linearGradient>
			</defs>

			{#each DB_GRID as db, idx}
				{@const y = 10 + ((idx + 1) / (DB_GRID.length + 1)) * (H - 26)}
				<line x1="0" y1={y} x2={W} y2={y} class="grid-line" />
				<text x={W - 4} y={y - 2} class="grid-text db-label">{db} dB</text>
			{/each}

			{#each FREQ_GRID as freq}
				{@const x = freqToX(freq, W)}
				<line x1={x} y1="0" x2={x} y2={H - 16} class="grid-line" />
				<text x={x} y={H - 4} class="grid-text freq-label">{formatFreq(freq)}</text>
			{/each}

			{#if spectrumPaths.area}
				<path d={spectrumPaths.area} fill="url(#clipSpecGrad)" />
			{/if}
			{#if spectrumPaths.peak}
				<path d={spectrumPaths.peak} class="spec-peak" stroke={fillColor} />
			{/if}
			{#if spectrumPaths.line}
				<path d={spectrumPaths.line} class="spec-line" stroke={fillColor} />
			{/if}
		</svg>
	</div>
</div>

<style>
	.audio-clip-view {
		display: flex;
		gap: 8px;
		height: 100%;
		padding: 8px;
		background: var(--bg);
		user-select: none;
	}

	.clip-sidebar {
		flex: none;
		width: 196px;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		padding: 8px;
		background: var(--surface);
		border: 1px solid var(--border);
		border-left: 2px solid var(--clip-color);
	}

	.clip-meta {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.clip-title {
		font-size: 11px;
		color: var(--text);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.clip-file {
		font-size: 9px;
		color: var(--dim);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.clip-stats {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 4px 8px;
		padding: 6px 0;
		border-top: 1px solid var(--border-subtle);
		border-bottom: 1px solid var(--border-subtle);
	}

	.stat-row {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
	}

	.stat-label {
		color: var(--dim);
	}

	.stat-val {
		color: var(--text);
		font-variant-numeric: tabular-nums;
	}

	.fade-controls {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.fade-row {
		display: grid;
		grid-template-columns: 52px 1fr 34px;
		align-items: center;
		gap: 4px;
		font-size: 9px;
	}

	.fade-label {
		color: var(--dim);
	}

	.fade-val {
		text-align: right;
		color: var(--text);
		font-variant-numeric: tabular-nums;
	}

	.fade-row input[type='range'] {
		width: 100%;
		accent-color: var(--clip-color);
	}

	.wave-pane,
	.spectrum-pane {
		position: relative;
		flex: 1;
		min-width: 200px;
		background: #090909;
		border: 1px solid var(--border);
		overflow: hidden;
	}

	.pane-label {
		position: absolute;
		top: 4px;
		left: 6px;
		right: 6px;
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		letter-spacing: 0.06em;
		color: var(--dim);
		z-index: 3;
		pointer-events: none;
	}

	.pane-mode {
		color: var(--clip-color);
	}

	.clip-wave-svg {
		position: absolute;
		inset: 18px 0 6px;
		width: 100%;
		height: calc(100% - 24px);
		opacity: 0.85;
	}

	.clip-fade-svg,
	.spectrum-svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		display: block;
	}

	.fade-shade {
		fill: rgba(0, 0, 0, 0.5);
	}

	.fade-line {
		fill: none;
		stroke: var(--text);
		stroke-width: 1.5;
		vector-effect: non-scaling-stroke;
	}

	.playhead-line {
		stroke: var(--text);
		stroke-width: 1.5;
		vector-effect: non-scaling-stroke;
	}

	.grid-line {
		stroke: var(--grid-minor);
		stroke-width: 1;
		shape-rendering: crispEdges;
	}

	.grid-text {
		fill: var(--dim);
		font-size: 8px;
	}

	.db-label {
		text-anchor: end;
	}

	.freq-label {
		text-anchor: middle;
	}

	.spec-line {
		fill: none;
		stroke-width: 1.5;
		vector-effect: non-scaling-stroke;
	}

	.spec-peak {
		fill: none;
		stroke-width: 1;
		stroke-opacity: 0.4;
		stroke-dasharray: 2 2;
		vector-effect: non-scaling-stroke;
	}

	.empty-msg {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		font-size: 10px;
		color: var(--dim);
	}
</style>
