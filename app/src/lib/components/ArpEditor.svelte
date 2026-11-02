<script lang="ts">
	/**
	 * The arpeggiator controls for one MIDI track, laid out for the Arp device:
	 * style and rate on the left, the gate, swing and velocity dials in the
	 * middle, and the transposed repeats on the right. Dials go through the
	 * session's per-frame queue after a checkpoint; every other control is one
	 * undo step. The engine rewrites the notes, so the piano roll keeps showing
	 * the drawn ones.
	 */

	import {
		ARP_RATES,
		ARP_STYLES,
		DECAY_SPEC,
		GATE_SPEC,
		MAX_DISTANCE,
		MAX_STEPS,
		MAX_VELOCITY,
		SWING_SPEC,
		TARGET_SPEC,
		targetVelocity,
		withArp
	} from '$lib/arp';
	import { session } from '$lib/session.svelte';
	import type { ArpRate, ArpStyle, ArpView, TrackView } from '$lib/wire';
	import Knob from './Knob.svelte';

	interface Props {
		track: TrackView;
		index: number;
	}

	let { track, index }: Props = $props();

	const arp = $derived(track.arp);

	function set(patch: Partial<ArpView>): void {
		session.setArp(index, withArp(arp, patch));
	}

	function queue(patch: Partial<ArpView>): void {
		session.queueArp(index, withArp(arp, patch));
	}

	function reseed(): void {
		set({ seed: Math.floor(Math.random() * 0x100000000) });
	}
</script>

<div class="arp">
	<div class="column">
		<label class="field">
			<span class="label">Style</span>
			<select
				class="select"
				aria-label="Arpeggiator style of {track.name}"
				value={arp.style}
				onchange={(event) => set({ style: event.currentTarget.value as ArpStyle })}
			>
				{#each ARP_STYLES as style (style.value)}
					<option value={style.value}>{style.label}</option>
				{/each}
			</select>
		</label>
		<label class="field">
			<span class="label">Rate</span>
			<select
				class="select"
				aria-label="Arpeggiator rate of {track.name}"
				value={arp.rate}
				onchange={(event) => set({ rate: event.currentTarget.value as ArpRate })}
			>
				{#each ARP_RATES as rate (rate)}
					<option value={rate}>{rate}</option>
				{/each}
			</select>
		</label>
		{#if arp.style === 'random'}
			<button type="button" class="reseed" onclick={reseed}>New seed</button>
		{/if}
	</div>

	<div class="knobs" role="group" aria-label="Arpeggiator timing and velocity of {track.name}">
		<Knob
			spec={GATE_SPEC}
			value={arp.gate}
			label="Gate of {track.name}'s arpeggiator"
			onbegin={() => session.dispatch('checkpoint')}
			oninput={(gate) => queue({ gate })}
		/>
		<Knob
			spec={SWING_SPEC}
			value={arp.swing}
			label="Swing of {track.name}'s arpeggiator"
			onbegin={() => session.dispatch('checkpoint')}
			oninput={(swing) => queue({ swing })}
		/>
		<Knob
			spec={DECAY_SPEC}
			value={arp.velocityDecay}
			label="Velocity decay of {track.name}'s arpeggiator"
			onbegin={() => session.dispatch('checkpoint')}
			oninput={(velocityDecay) => queue({ velocityDecay })}
		/>
		<Knob
			spec={TARGET_SPEC}
			value={arp.velocityTarget / MAX_VELOCITY}
			label="Velocity target of {track.name}'s arpeggiator"
			onbegin={() => session.dispatch('checkpoint')}
			oninput={(share) => queue({ velocityTarget: targetVelocity(share) })}
		/>
	</div>

	<div class="column">
		<label class="field">
			<span class="label">Distance</span>
			<input
				class="number"
				type="number"
				min={-MAX_DISTANCE}
				max={MAX_DISTANCE}
				step="1"
				aria-label="Semitones between repeats of {track.name}'s arpeggiator"
				value={arp.distance}
				onchange={(event) => set({ distance: Number(event.currentTarget.value) })}
			/>
		</label>
		<label class="field">
			<span class="label">Steps</span>
			<input
				class="number"
				type="number"
				min="0"
				max={MAX_STEPS}
				step="1"
				aria-label="Transposed repeats of {track.name}'s arpeggiator"
				value={arp.steps}
				onchange={(event) => set({ steps: Number(event.currentTarget.value) })}
			/>
		</label>
	</div>
</div>

<style>
	.arp {
		display: flex;
		align-items: flex-start;
		gap: 16px;
	}

	.column {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 96px;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.label {
		font-size: 9px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.select,
	.number {
		height: 22px;
		padding: 0 4px;
		font-size: 11px;
		color: var(--text);
		background: var(--bg);
		border: 1px solid var(--border-subtle);
	}

	.select:focus-visible,
	.number:focus-visible {
		outline: 1px solid var(--green);
	}

	.knobs {
		display: flex;
		align-items: flex-start;
		gap: 8px;
	}

	.reseed {
		height: 22px;
		padding: 0 6px;
		font-size: 9px;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--text-subtle);
		background: transparent;
		border: 1px solid var(--border-subtle);
	}

	.reseed:hover {
		filter: none;
		color: var(--text);
		border-color: var(--green);
	}
</style>
