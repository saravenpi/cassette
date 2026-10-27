<script lang="ts">
	/**
	 * The voice editor for one MIDI track, laid out for the Instrument device: the
	 * preset and its oscillator with the envelope on the left, and gain, octave
	 * and the four envelope stages on the right, each sent through
	 * `setInstrument` at most once per frame, with the envelope drawn from the
	 * values the sliders show.
	 */

	import { envelopePoints, rangePercent } from '$lib/geometry';
	import { session } from '$lib/session.svelte';
	import type { Voice } from '$lib/session.svelte';
	import type { ParamSpecView, TrackView } from '$lib/wire';
	import Knob from './Knob.svelte';

	const CUTOFF_SPEC: ParamSpecView = {
		name: 'Cutoff',
		min: 20,
		max: 20000,
		default: 20000,
		unit: 'hertz',
		log: true,
		choices: []
	};

	const RESONANCE_SPEC: ParamSpecView = {
		name: 'Reson',
		min: 0,
		max: 1,
		default: 0,
		unit: 'percent',
		log: false,
		choices: []
	};

	const DETUNE_SPEC: ParamSpecView = {
		name: 'Detune',
		min: 0,
		max: 1,
		default: 0,
		unit: 'percent',
		log: false,
		choices: []
	};

	const SUB_SPEC: ParamSpecView = {
		name: 'Sub',
		min: 0,
		max: 1,
		default: 0,
		unit: 'percent',
		log: false,
		choices: []
	};

	const WIDTH = 240;
	const HEIGHT = 52;
	const SUSTAIN_WIDTH = 72;
	const DEFAULT_GAIN = 0.45;


	const SLIDERS: { id: keyof Voice; label: string; min: number; max: number; step: number; unit: string; default: number }[] =
		[
			{ id: 'gain', label: 'Gain', min: 0, max: 1, step: 0.01, unit: '', default: 0.45 },
			{ id: 'octave', label: 'Octave', min: -3, max: 3, step: 1, unit: '', default: 0 },
			{ id: 'attack', label: 'Attack', min: 0, max: 2, step: 0.005, unit: 's', default: 0.01 },
			{ id: 'decay', label: 'Decay', min: 0, max: 2, step: 0.005, unit: 's', default: 0.12 },
			{ id: 'sustain', label: 'Sustain', min: 0, max: 1, step: 0.01, unit: '', default: 0.6 },
			{ id: 'release', label: 'Release', min: 0, max: 3, step: 0.005, unit: 's', default: 0.15 }
		];

	interface Props {
		track: TrackView;
		index: number;
	}

	let { track, index }: Props = $props();

	let editing = $state(false);
	let gain = $state(DEFAULT_GAIN);
	let voice = $state<Omit<Voice, 'gain'>>({
		octave: 0,
		attack: 0.01,
		decay: 0.12,
		sustain: 0.6,
		release: 0.15
	});

	const envelope = $derived(
		envelopePoints(voice.attack, voice.decay, voice.sustain, voice.release, WIDTH, HEIGHT, SUSTAIN_WIDTH)
	);

	$effect(() => {
		if (editing) return;
		gain = track.gain;
		voice = {
			octave: track.octave,
			attack: track.attack,
			decay: track.decay,
			sustain: track.sustain,
			release: track.release
		};
	});

	function valueOf(id: keyof Voice): number {
		switch (id) {
			case 'gain':
				return gain;
			case 'octave':
				return voice.octave;
			case 'attack':
				return voice.attack;
			case 'decay':
				return voice.decay;
			case 'sustain':
				return voice.sustain;
			case 'release':
				return voice.release;
		}
	}

	function onSlider(id: keyof Voice, raw: string): void {
		const value = Number(raw);
		editing = true;
		if (id === 'gain') gain = value;
		else if (id === 'octave') voice = { ...voice, octave: value };
		else if (id === 'attack') voice = { ...voice, attack: value };
		else if (id === 'decay') voice = { ...voice, decay: value };
		else if (id === 'sustain') voice = { ...voice, sustain: value };
		else voice = { ...voice, release: value };
		session.queueVoice(index, { gain, ...voice });
	}

	function resetSlider(id: keyof Voice): void {
		const spec = SLIDERS.find((s) => s.id === id);
		if (!spec) return;
		session.dispatch('checkpoint');
		onSlider(id, String(spec.default));
		editing = false;
	}

	function onToneChange(which: 'cutoff' | 'resonance' | 'detune' | 'sub', value: number): void {
		const cutoff = which === 'cutoff' ? value : track.cutoff;
		const resonance = which === 'resonance' ? value : track.resonance;
		const detune = which === 'detune' ? value : track.detune;
		const sub = which === 'sub' ? value : track.sub;
		session.queueInstrumentTone(index, cutoff, resonance, detune, sub);
	}
</script>

<div class="voice">
	<div class="shape">
		<p class="preset">
			<span class="strong">{track.preset}</span> on a {track.waveform.toLowerCase()} oscillator
		</p>
		<h3 class="eyebrow">Envelope</h3>
		<svg
			class="envelope"
			viewBox="0 0 {WIDTH} {HEIGHT}"
			preserveAspectRatio="none"
			role="img"
			aria-label="Attack, decay, sustain and release envelope"
		>
			{#each [0.25, 0.5, 0.75] as level (level)}
				<line class="guide" x1="0" x2={WIDTH} y1={HEIGHT * level} y2={HEIGHT * level} />
			{/each}
			<polyline points={envelope} />
		</svg>
	</div>

	<div class="sliders" role="group" aria-label="Voice of {track.name}">
		{#each SLIDERS as slider (slider.id)}
			<label class="slider">
				<span class="label">{slider.label}</span>
				<input
					type="range"
					min={slider.min}
					max={slider.max}
					step={slider.step}
					value={valueOf(slider.id)}
					style="--filled: {rangePercent(valueOf(slider.id), slider.min, slider.max)}"
					oninput={(event) => onSlider(slider.id, event.currentTarget.value)}
					onpointerdown={(event) => {
						if (event.detail === 2) {
							resetSlider(slider.id);
						} else {
							editing = true;
						}
					}}
					ondblclick={() => resetSlider(slider.id)}
					onfocus={() => (editing = true)}
					onblur={() => (editing = false)}
				/>
				<span class="value">
					{valueOf(slider.id).toFixed(slider.step < 1 ? 2 : 0)}{slider.unit}
				</span>
			</label>
		{/each}
	</div>

	<div class="tone-section">
		<h3 class="eyebrow">Tone</h3>
		<div class="tone-knobs" role="group" aria-label="Tone of {track.name}">
			<Knob
				spec={CUTOFF_SPEC}
				value={track.cutoff}
				label="Cutoff of {track.name}"
				onbegin={() => session.dispatch('checkpoint')}
				oninput={(val) => onToneChange('cutoff', val)}
			/>
			<Knob
				spec={RESONANCE_SPEC}
				value={track.resonance}
				label="Resonance of {track.name}"
				onbegin={() => session.dispatch('checkpoint')}
				oninput={(val) => onToneChange('resonance', val)}
			/>
			<Knob
				spec={DETUNE_SPEC}
				value={track.detune}
				label="Detune of {track.name}"
				onbegin={() => session.dispatch('checkpoint')}
				oninput={(val) => onToneChange('detune', val)}
			/>
			<Knob
				spec={SUB_SPEC}
				value={track.sub}
				label="Sub oscillator of {track.name}"
				onbegin={() => session.dispatch('checkpoint')}
				oninput={(val) => onToneChange('sub', val)}
			/>
		</div>
	</div>
</div>

<style>
	.voice {
		display: grid;
		grid-template-columns: 180px 240px auto;
		gap: 16px;
		align-items: start;
	}

	.tone-section {
		display: flex;
		flex-direction: column;
	}

	.tone-knobs {
		display: flex;
		align-items: flex-start;
		gap: 8px;
	}

	.preset {
		margin-bottom: 10px;
		font-size: 11px;
		line-height: 1.5;
		color: var(--dim);
	}

	.strong {
		color: var(--text);
	}

	.eyebrow {
		margin: 0 0 8px;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.envelope {
		display: block;
		width: 100%;
		height: 64px;
		background: var(--bg);
		box-shadow: inset 0 0 0 1px var(--border-subtle);
	}

	.guide {
		stroke: var(--grid-minor);
		stroke-width: 1;
		vector-effect: non-scaling-stroke;
	}

	.envelope polyline {
		fill: none;
		stroke: var(--green);
		stroke-width: 1.5;
		vector-effect: non-scaling-stroke;
	}

	.sliders {
		display: flex;
		flex-direction: column;
	}

	.slider {
		display: grid;
		grid-template-columns: 60px 1fr 48px;
		align-items: center;
		gap: 10px;
		height: 28px;
		border-bottom: 1px solid var(--border-subtle);
		font-size: 11px;
	}

	.slider:first-child {
		border-top: 1px solid var(--border-subtle);
	}

	.label {
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.value {
		text-align: right;
		color: var(--text);
		font-variant-numeric: tabular-nums;
	}

	input[type='range'] {
		width: 100%;
	}
</style>
