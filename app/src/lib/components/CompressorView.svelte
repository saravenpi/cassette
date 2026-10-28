<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { specFor } from '$lib/chain';
	import { session } from '$lib/session.svelte';
	import { MASTER_TRACK, type EffectView } from '$lib/wire';
	import Knob from './Knob.svelte';

	const HISTORY_LEN = 90;
	const MIN_DB = -60;
	const MAX_DB = 0;

	interface Props {
		effect: EffectView;
		index: number;
		at: number;
	}

	let { effect, index, at }: Props = $props();

	let viewMode = $state<'split' | 'curve' | 'activity'>('split');
	let inEnvDb = $state(MIN_DB);
	let grDb = $state(0);
	let historyIn = $state<number[]>(new Array(HISTORY_LEN).fill(MIN_DB));
	let historyGr = $state<number[]>(new Array(HISTORY_LEN).fill(0));
	let draggingKnee = $state(false);
	let curveSvg = $state<SVGSVGElement | null>(null);
	let animId: number | null = null;
	let lastTick = performance.now();

	const catalog = $derived(session.snapshot?.effectCatalog ?? []);
	const spec = $derived(specFor(catalog, effect.kind));
	const tracks = $derived(session.snapshot?.tracks ?? []);

	const threshold = $derived(effect.params[0] ?? -18);
	const ratio = $derived(Math.max(1, effect.params[1] ?? 4));
	const attackMs = $derived(Math.max(0.1, effect.params[2] ?? 10));
	const releaseMs = $derived(Math.max(5, effect.params[3] ?? 100));
	const makeup = $derived(effect.params[4] ?? 0);

	const detectorLevel = $derived.by(() => {
		if (effect.sidechain !== null && effect.sidechain !== undefined) {
			return session.snapshot?.tracks[effect.sidechain]?.level ?? 0;
		}
		if (index === MASTER_TRACK) {
			return session.snapshot?.masterLevel ?? 0;
		}
		return session.snapshot?.tracks[index]?.level ?? 0;
	});

	function computeOutDb(inDb: number): number {
		const compressed =
			inDb <= threshold ? inDb : threshold + (inDb - threshold) / Math.max(1, ratio);
		return Math.min(6, compressed + makeup);
	}

	function dbToNorm(db: number): number {
		return Math.max(0, Math.min(1, (db - MIN_DB) / (MAX_DB - MIN_DB)));
	}

	function tick(): void {
		const now = performance.now();
		const dt = Math.max(0.001, Math.min(0.1, (now - lastTick) / 1000));
		lastTick = now;

		const rawLevel = detectorLevel;
		const targetInDb =
			rawLevel > 0.0003 ? Math.max(MIN_DB, Math.min(MAX_DB, 20 * Math.log10(rawLevel))) : MIN_DB;

		const atkCoeff = 1 - Math.exp(-dt / (attackMs * 0.001));
		const relCoeff = 1 - Math.exp(-dt / (releaseMs * 0.001));
		const coeff = targetInDb > inEnvDb ? atkCoeff : relCoeff;
		inEnvDb = inEnvDb + (targetInDb - inEnvDb) * Math.max(0.08, coeff);

		let targetGr = 0;
		if (effect.enabled && inEnvDb > threshold) {
			targetGr = (inEnvDb - threshold) * (1 - 1 / Math.max(1, ratio));
		}
		grDb = grDb + (targetGr - grDb) * Math.max(0.12, coeff);

		const nextIn = historyIn.slice(1);
		nextIn.push(inEnvDb);
		historyIn = nextIn;

		const nextGr = historyGr.slice(1);
		nextGr.push(grDb);
		historyGr = nextGr;

		animId = requestAnimationFrame(tick);
	}

	onMount(() => {
		lastTick = performance.now();
		animId = requestAnimationFrame(tick);
	});

	onDestroy(() => {
		if (animId !== null) cancelAnimationFrame(animId);
		stopKneeDrag();
	});

	const CW = 136;
	const CH = 136;

	const transferPath = $derived.by(() => {
		const pts: string[] = [];
		for (let i = 0; i <= 40; i++) {
			const db = MIN_DB + (i / 40) * (MAX_DB - MIN_DB);
			const out = computeOutDb(db);
			const x = dbToNorm(db) * CW;
			const y = (1 - dbToNorm(out)) * CH;
			pts.push(`${i === 0 ? 'M' : 'L'} ${x.toFixed(1)} ${y.toFixed(1)}`);
		}
		return pts.join(' ');
	});

	const kneePos = $derived({
		x: dbToNorm(threshold) * CW,
		y: (1 - dbToNorm(computeOutDb(threshold))) * CH
	});

	const ballPos = $derived({
		x: dbToNorm(inEnvDb) * CW,
		y: (1 - dbToNorm(computeOutDb(inEnvDb))) * CH
	});

	const AW = 200;
	const AH = 136;

	const activityInPath = $derived.by(() => {
		let d = `M 0 ${AH}`;
		for (let i = 0; i < HISTORY_LEN; i++) {
			const x = (i / (HISTORY_LEN - 1)) * AW;
			const y = (1 - dbToNorm(historyIn[i] ?? MIN_DB)) * AH;
			d += ` L ${x.toFixed(1)} ${y.toFixed(1)}`;
		}
		d += ` L ${AW} ${AH} Z`;
		return d;
	});

	const activityGrPath = $derived.by(() => {
		let d = '';
		for (let i = 0; i < HISTORY_LEN; i++) {
			const x = (i / (HISTORY_LEN - 1)) * AW;
			const grNorm = Math.min(1, (historyGr[i] ?? 0) / 24);
			const y = 6 + grNorm * (AH - 12);
			d += `${i === 0 ? 'M' : ' L'} ${x.toFixed(1)} ${y.toFixed(1)}`;
		}
		return d;
	});

	function onKneeDown(event: PointerEvent): void {
		event.preventDefault();
		event.stopPropagation();
		session.dispatch('checkpoint');
		draggingKnee = true;
		window.addEventListener('pointermove', onKneeMove);
		window.addEventListener('pointerup', stopKneeDrag);
		window.addEventListener('pointercancel', stopKneeDrag);
	}

	function onKneeMove(event: PointerEvent): void {
		if (!draggingKnee || curveSvg === null) return;
		const rect = curveSvg.getBoundingClientRect();
		const normX = Math.max(0, Math.min(1, (event.clientX - rect.left) / Math.max(1, rect.width)));
		const nextThresh = +(MIN_DB + normX * (MAX_DB - MIN_DB)).toFixed(1);
		session.queueEffectParam(index, at, 0, nextThresh);
	}

	function stopKneeDrag(): void {
		draggingKnee = false;
		window.removeEventListener('pointermove', onKneeMove);
		window.removeEventListener('pointerup', stopKneeDrag);
		window.removeEventListener('pointercancel', stopKneeDrag);
	}

	function onSidechainChange(event: Event): void {
		const raw = (event.currentTarget as HTMLSelectElement).value;
		const source = raw === '' ? null : Number(raw);
		session.dispatch({ setEffectSidechain: { track: index, index: at, source } });
	}
</script>

<div class="compressor-view" role="region" aria-label="Compressor live visualizer">
	<div class="vis-container">
		<div class="vis-toolbar">
			<button
				type="button"
				class="mode-btn"
				class:active={viewMode === 'split'}
				onclick={() => (viewMode = 'split')}
			>
				DUAL
			</button>
			<button
				type="button"
				class="mode-btn"
				class:active={viewMode === 'curve'}
				onclick={() => (viewMode = 'curve')}
			>
				CURVE
			</button>
			<button
				type="button"
				class="mode-btn"
				class:active={viewMode === 'activity'}
				onclick={() => (viewMode = 'activity')}
			>
				GR TRACE
			</button>
			<span class="gr-readout">GR -{grDb.toFixed(1)} dB</span>
		</div>

		<div class="vis-body">
			{#if viewMode === 'split' || viewMode === 'curve'}
				<div class="curve-box" class:wide={viewMode === 'curve'}>
					<svg bind:this={curveSvg} viewBox="0 0 {CW} {CH}" class="vis-svg">
						{#each [-48, -36, -24, -12] as db}
							{@const p = dbToNorm(db) * CW}
							{@const py = (1 - dbToNorm(db)) * CH}
							<line x1={p} y1="0" x2={p} y2={CH} class="grid-line" />
							<line x1="0" y1={py} x2={CW} y2={py} class="grid-line" />
						{/each}
						<line x1="0" y1={CH} x2={CW} y2="0" class="unity-line" />
						<line x1={kneePos.x} y1="0" x2={kneePos.x} y2={CH} class="thresh-line" />
						<path d={transferPath} class="transfer-curve" />
						{#if inEnvDb > MIN_DB + 0.5}
							<circle cx={ballPos.x} cy={ballPos.y} r="4" class="signal-ball" />
						{/if}
						<circle
							cx={kneePos.x}
							cy={kneePos.y}
							r="5.5"
							class="knee-node"
							role="slider"
							tabindex="0"
							aria-label="Compressor threshold"
							aria-valuenow={threshold}
							onpointerdown={onKneeDown}
						/>
						<text x="4" y="10" class="vis-caption">TRANSFER</text>
					</svg>
				</div>
			{/if}

			{#if viewMode === 'split' || viewMode === 'activity'}
				<div class="activity-box">
					<svg viewBox="0 0 {AW} {AH}" preserveAspectRatio="none" class="vis-svg">
						{#each [0.25, 0.5, 0.75] as frac}
							<line x1="0" y1={frac * AH} x2={AW} y2={frac * AH} class="grid-line" />
						{/each}
						<path d={activityInPath} class="in-area" />
						<path d={activityGrPath} class="gr-trace" />
					</svg>
					<span class="overlay-caption">GAIN REDUCTION</span>
				</div>
			{/if}

			<div class="gr-meter" title="Gain reduction: -{grDb.toFixed(1)} dB">
				<span class="gr-label">GR</span>
				<div class="gr-track">
					<div
						class="gr-fill"
						style="height: {Math.min(100, (grDb / 24) * 100).toFixed(1)}%"
					></div>
				</div>
			</div>
		</div>
	</div>

	<div class="comp-controls">
		<div class="knobs-grid">
			{#if spec}
				{#each spec.params as paramSpec, p (paramSpec.name)}
					<Knob
						spec={paramSpec}
						value={effect.params[p] ?? paramSpec.default}
						label="{effect.name} {paramSpec.name}"
						onbegin={() => session.dispatch('checkpoint')}
						oninput={(val) => session.queueEffectParam(index, at, p, val)}
					/>
				{/each}
			{/if}
		</div>

		<label class="sc-row">
			<span class="sc-label">SIDECHAIN</span>
			<select
				class="sc-select"
				value={effect.sidechain === null ? '' : String(effect.sidechain)}
				onchange={onSidechainChange}
			>
				<option value="">None (Self)</option>
				{#each tracks as candidate, tIndex (tIndex)}
					{#if tIndex !== index}
						<option value={String(tIndex)}>{tIndex + 1}. {candidate.name}</option>
					{/if}
				{/each}
			</select>
		</label>
	</div>
</div>

<style>
	.compressor-view {
		display: flex;
		gap: 8px;
		height: 100%;
		min-width: 520px;
		user-select: none;
	}

	.vis-container {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 280px;
	}

	.vis-toolbar {
		display: flex;
		align-items: center;
		gap: 3px;
		height: 20px;
	}

	.mode-btn {
		height: 18px;
		padding: 0 6px;
		font-size: 9px;
		background: var(--surface);
		border: 1px solid var(--border);
		color: var(--dim);
		cursor: pointer;
	}

	.mode-btn:hover {
		color: var(--text);
		border-color: var(--text-subtle);
	}

	.mode-btn.active {
		background: var(--green);
		border-color: var(--green);
		color: var(--on-green);
	}

	.gr-readout {
		margin-left: auto;
		font-size: 9px;
		color: var(--amber);
		font-variant-numeric: tabular-nums;
	}

	.vis-body {
		flex: 1;
		display: flex;
		gap: 4px;
		min-height: 100px;
	}

	.curve-box {
		position: relative;
		width: 136px;
		flex: none;
		background: #090909;
		border: 1px solid var(--border);
		overflow: hidden;
	}

	.curve-box.wide {
		flex: 1;
		width: auto;
	}

	.activity-box {
		position: relative;
		flex: 1;
		background: #090909;
		border: 1px solid var(--border);
		overflow: hidden;
	}

	.vis-svg {
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

	.unity-line {
		stroke: var(--grid-major);
		stroke-width: 1;
		stroke-dasharray: 2 2;
	}

	.thresh-line {
		stroke: var(--amber);
		stroke-width: 1;
		stroke-opacity: 0.45;
		stroke-dasharray: 2 2;
	}

	.transfer-curve {
		fill: none;
		stroke: var(--green);
		stroke-width: 1.75;
	}

	.signal-ball {
		fill: var(--text);
		stroke: var(--green);
		stroke-width: 1.5;
	}

	.knee-node {
		fill: var(--surface);
		stroke: var(--amber);
		stroke-width: 1.75;
		cursor: ew-resize;
	}

	.knee-node:hover {
		fill: var(--amber);
	}

	.vis-caption,
	.overlay-caption {
		font-size: 8px;
		fill: var(--dim);
		color: var(--dim);
		letter-spacing: 0.05em;
		pointer-events: none;
	}

	.overlay-caption {
		position: absolute;
		top: 3px;
		left: 5px;
	}

	.in-area {
		fill: rgba(255, 255, 255, 0.12);
		stroke: rgba(255, 255, 255, 0.28);
		stroke-width: 1;
		vector-effect: non-scaling-stroke;
	}

	.gr-trace {
		fill: none;
		stroke: var(--amber);
		stroke-width: 1.75;
		vector-effect: non-scaling-stroke;
	}

	.gr-meter {
		width: 18px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 3px;
		padding: 3px 2px;
		background: #090909;
		border: 1px solid var(--border);
	}

	.gr-label {
		font-size: 8px;
		color: var(--dim);
	}

	.gr-track {
		position: relative;
		flex: 1;
		width: 6px;
		background: var(--surface);
		overflow: hidden;
	}

	.gr-fill {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		background: var(--amber);
	}

	.comp-controls {
		flex: none;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		gap: 6px;
		padding: 6px;
		background: var(--surface);
		border: 1px solid var(--border-subtle);
	}

	.knobs-grid {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.sc-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		padding-top: 4px;
		border-top: 1px solid var(--border-subtle);
		font-size: 9px;
	}

	.sc-label {
		color: var(--dim);
		letter-spacing: 0.04em;
	}

	.sc-select {
		flex: 1;
		height: 20px;
		background: var(--bg);
		color: var(--text);
		border: 1px solid var(--border);
		font-family: inherit;
		font-size: 9px;
		padding: 0 4px;
	}
</style>
