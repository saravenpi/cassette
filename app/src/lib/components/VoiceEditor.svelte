<script lang="ts">
	/**
	 * The voice editor for one MIDI track: gain, octave and the four envelope
	 * stages, each sent through `setInstrument` at most once per frame, with the
	 * envelope drawn from the values the sliders show.
	 */

	import { envelopePoints } from '$lib/geometry';
	import { session } from '$lib/session.svelte';
	import type { Voice } from '$lib/session.svelte';
	import type { TrackView } from '$lib/wire';

	const WIDTH = 240;
	const HEIGHT = 52;
	const SUSTAIN_WIDTH = 72;
	const DEFAULT_GAIN = 0.45;


	const SLIDERS: { id: keyof Voice; label: string; min: number; max: number; step: number; unit: string }[] =
		[
			{ id: 'gain', label: 'Gain', min: 0, max: 1, step: 0.01, unit: '' },
			{ id: 'octave', label: 'Octave', min: -3, max: 3, step: 1, unit: '' },
			{ id: 'attack', label: 'Attack', min: 0, max: 2, step: 0.005, unit: 's' },
			{ id: 'decay', label: 'Decay', min: 0, max: 2, step: 0.005, unit: 's' },
			{ id: 'sustain', label: 'Sustain', min: 0, max: 1, step: 0.01, unit: '' },
			{ id: 'release', label: 'Release', min: 0, max: 3, step: 0.005, unit: 's' }
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
		if (id === 'gain') gain = value;
		else if (id === 'octave') voice = { ...voice, octave: value };
		else if (id === 'attack') voice = { ...voice, attack: value };
		else if (id === 'decay') voice = { ...voice, decay: value };
		else if (id === 'sustain') voice = { ...voice, sustain: value };
		else voice = { ...voice, release: value };
		session.queueVoice(index, { gain, ...voice });
	}
</script>

<svg
	class="envelope"
	viewBox="0 0 {WIDTH} {HEIGHT}"
	preserveAspectRatio="none"
	role="img"
	aria-label="Attack, decay, sustain and release envelope"
>
	<polyline points={envelope} />
</svg>

<div class="sliders">
	{#each SLIDERS as slider (slider.id)}
		<label class="slider">
			<span class="label">{slider.label}</span>
			<input
				type="range"
				min={slider.min}
				max={slider.max}
				step={slider.step}
				value={valueOf(slider.id)}
				oninput={(event) => onSlider(slider.id, event.currentTarget.value)}
				onpointerdown={() => (editing = true)}
				onchange={() => (editing = false)}
				onblur={() => (editing = false)}
			/>
			<span class="value">
				{valueOf(slider.id).toFixed(slider.step < 1 ? 2 : 0)}{slider.unit}
			</span>
		</label>
	{/each}
</div>

<style>
	.envelope {
		display: block;
		width: 100%;
		height: 52px;
		background: var(--bg);
		border: 1px solid var(--line);
		border-radius: 0;
	}

	.envelope polyline {
		fill: none;
		stroke: var(--head);
		stroke-width: 1.5;
		vector-effect: non-scaling-stroke;
	}

	.sliders {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-top: 10px;
	}

	.slider {
		display: grid;
		grid-template-columns: 52px 1fr 44px;
		align-items: center;
		gap: 6px;
		font-size: 10px;
	}

	.label,
	.value {
		color: var(--dim);
	}

	.value {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}

	input[type='range'] {
		width: 100%;
		height: 14px;
		margin: 0;
		accent-color: var(--accent);
	}
</style>
