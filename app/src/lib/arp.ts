/**
 * The arpeggiator device's choices, dial specs and range rules, kept in step
 * with `src/models/arp.rs`. Every function here is pure.
 */

import type { ArpRate, ArpStyle, ArpView, ParamSpecView } from './wire';

/** Widest transposition between repeats, in semitones, either way. */
export const MAX_DISTANCE = 24;
/** Most transposed repeats after the first pass. */
export const MAX_STEPS = 8;
/** Hardest MIDI velocity, the top of the velocity target. */
export const MAX_VELOCITY = 127;

/** The settings a fresh track starts with: off, sixteenths, an octave apart. */
export const DEFAULT_ARP: ArpView = {
	enabled: false,
	style: 'up',
	rate: '1/16',
	gate: 0.8,
	distance: 12,
	steps: 0,
	swing: 0,
	velocityDecay: 0,
	velocityTarget: 0,
	seed: 0
};

/** The styles in menu order, with their labels. */
export const ARP_STYLES: { value: ArpStyle; label: string }[] = [
	{ value: 'up', label: 'Up' },
	{ value: 'down', label: 'Down' },
	{ value: 'upDown', label: 'Up Down' },
	{ value: 'downUp', label: 'Down Up' },
	{ value: 'converge', label: 'Converge' },
	{ value: 'diverge', label: 'Diverge' },
	{ value: 'playOrder', label: 'Play Order' },
	{ value: 'random', label: 'Random' }
];

/** The step rates in menu order, straight values then triplets. */
export const ARP_RATES: ArpRate[] = ['1/1', '1/2', '1/4', '1/8', '1/16', '1/32', '1/4T', '1/8T', '1/16T'];

export const GATE_SPEC: ParamSpecView = {
	name: 'Gate',
	min: 0.01,
	max: 2,
	default: 0.8,
	unit: 'percent',
	log: false,
	choices: []
};

export const SWING_SPEC: ParamSpecView = {
	name: 'Swing',
	min: 0,
	max: 0.75,
	default: 0,
	unit: 'percent',
	log: false,
	choices: []
};

export const DECAY_SPEC: ParamSpecView = {
	name: 'Vel Decay',
	min: 0,
	max: 16,
	default: 0,
	unit: 'beats',
	log: false,
	choices: []
};

/** The velocity target on a dial, as a share of the hardest velocity. */
export const TARGET_SPEC: ParamSpecView = {
	name: 'Target',
	min: 0,
	max: 1,
	default: 0,
	unit: 'percent',
	log: false,
	choices: []
};

function clamp(value: number, low: number, high: number): number {
	return Math.min(high, Math.max(low, value));
}

function whole(value: number, low: number, high: number, fallback: number): number {
	return Number.isFinite(value) ? clamp(Math.round(value), low, high) : fallback;
}

function real(value: number, low: number, high: number, fallback: number): number {
	return Number.isFinite(value) ? clamp(value, low, high) : fallback;
}

/** The settings with `patch` applied, every number held inside the engine's range. */
export function withArp(arp: ArpView, patch: Partial<ArpView>): ArpView {
	const next = { ...arp, ...patch };
	return {
		...next,
		gate: real(next.gate, GATE_SPEC.min, GATE_SPEC.max, DEFAULT_ARP.gate),
		distance: whole(next.distance, -MAX_DISTANCE, MAX_DISTANCE, DEFAULT_ARP.distance),
		steps: whole(next.steps, 0, MAX_STEPS, DEFAULT_ARP.steps),
		swing: real(next.swing, SWING_SPEC.min, SWING_SPEC.max, DEFAULT_ARP.swing),
		velocityDecay: real(next.velocityDecay, DECAY_SPEC.min, DECAY_SPEC.max, DEFAULT_ARP.velocityDecay),
		velocityTarget: whole(next.velocityTarget, 0, MAX_VELOCITY, DEFAULT_ARP.velocityTarget),
		seed: whole(next.seed, 0, 0xffffffff, DEFAULT_ARP.seed)
	};
}

/** The velocity target a `TARGET_SPEC` dial position stands for. */
export function targetVelocity(share: number): number {
	return whole(share * MAX_VELOCITY, 0, MAX_VELOCITY, DEFAULT_ARP.velocityTarget);
}
