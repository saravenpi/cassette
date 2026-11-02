/**
 * The oscillator picture of the voice editor: one cycle of a track's voice drawn
 * from its settings, mirroring the engine's `Voice::next_sample` in
 * `src/core/synth.rs`, and the helpers that hold a live scope still.
 */

import type { TrackView } from './wire';

/** The instrument settings that shape the oscillator. */
export type VoiceParams = Pick<
	TrackView,
	'waveform' | 'unison' | 'detune' | 'sub' | 'cutoff' | 'resonance' | 'fmRatio' | 'fmIndex'
>;

/** Pitch the static shape is drawn at, so the filter cutoff reads against a real note. */
const REFERENCE_HZ = 220;
/** Seed the engine starts its noise generator from. */
const NOISE_SEED = 0x12345678;
/** Cutoff at or above which the engine leaves the voice unfiltered. */
const OPEN_CUTOFF = 20000;
/** Peak below which a scope reading counts as silence. */
export const SILENCE = 1e-4;

const TWO_PI = Math.PI * 2;

function clamp(value: number, min: number, max: number): number {
	return Math.min(max, Math.max(min, value));
}

/** A xorshift32 noise source in [-1, 1], the engine's generator bit for bit. */
function noiseSource(seed: number): () => number {
	let state = seed >>> 0;
	return () => {
		state = (state ^ (state << 13)) >>> 0;
		state = (state ^ (state >>> 17)) >>> 0;
		state = (state ^ (state << 5)) >>> 0;
		return (state / 0xffffffff) * 2 - 1;
	};
}

/** One waveform sample at `phase` in [0, 1), as the engine's `waveform` computes it. */
function oscillator(shape: string, phase: number, noise: () => number): number {
	switch (shape) {
		case 'square':
			return phase < 0.5 ? 1 : -1;
		case 'saw':
			return 2 * phase - 1;
		case 'triangle':
			return phase < 0.5 ? 4 * phase - 1 : 3 - 4 * phase;
		case 'pulse':
			return phase < 0.25 ? 1 : -1;
		case 'noise':
			return noise();
		default:
			return Math.sin(phase * TWO_PI);
	}
}

/** The engine's resonant low-pass (a TPT state-variable filter) at one sample rate. */
function lowPass(cutoff: number, resonance: number, sampleRate: number): (input: number) => number {
	const hz = clamp(clamp(cutoff, 20, OPEN_CUTOFF), 20, sampleRate * 0.49);
	const g = Math.tan((Math.PI * hz) / sampleRate);
	const k = 2 * (1 - clamp(resonance, 0, 1) * 0.98);
	const denom = 1 + k * g + g * g;
	let s1 = 0;
	let s2 = 0;
	return (input) => {
		const hp = (input - (k + g) * s1 - s2) / denom;
		const bp = g * hp + s1;
		const lp = g * bp + s2;
		s1 = 2 * bp - s1;
		s2 = 2 * lp - s2;
		return lp;
	};
}

/** Scale `values` so the largest magnitude is 1; silence stays silent. */
export function normalize(values: number[]): number[] {
	const peak = peakOf(values);
	return peak > 0 ? values.map((value) => value / peak) : values;
}

/** The largest magnitude in `values`. */
export function peakOf(values: number[]): number {
	let peak = 0;
	for (const value of values) peak = Math.max(peak, Math.abs(value));
	return peak;
}

/**
 * `points` samples in [-1, 1] of the voice's oscillator: one cycle, or two when
 * the sub oscillator sounds so its octave-down sine shows. Unison copies start
 * spread over the cycle and detuned as the engine starts them, FM bends the
 * phase, and a filter below 20 kHz runs warmed up so the cycle is its steady
 * state. The result is normalized to a peak of 1.
 */
export function voiceShape(voice: VoiceParams, points = 256): number[] {
	const count = Math.max(2, Math.round(points));
	const shape = voice.waveform.toLowerCase();
	const sub = clamp(voice.sub, 0, 1);
	const cycles = sub > 0 ? 2 : 1;
	const perCycle = count / cycles;
	const unison = clamp(Math.round(voice.unison), 1, 7);
	const detune = clamp(voice.detune, 0, 100);
	const copies = unison > 1 && detune > 0 ? unison : 1;
	const ratios = Array.from({ length: copies }, (_, i) =>
		copies > 1 ? 2 ** (((-0.5 + i / (copies - 1)) * detune) / 1200) : 1
	);
	const fmIndex = clamp(voice.fmIndex, 0, 10);
	const fmRatio = clamp(voice.fmRatio, 0.01, 32);
	const filter =
		voice.cutoff < OPEN_CUTOFF
			? lowPass(voice.cutoff, voice.resonance, REFERENCE_HZ * perCycle)
			: null;
	const warmup = filter ? count * 2 : 0;
	const noise = noiseSource(NOISE_SEED);
	const values: number[] = [];
	for (let n = 0; n < warmup + count; n += 1) {
		const t = n / perCycle;
		const bend = fmIndex > 0 ? (Math.sin(t * fmRatio * TWO_PI) * fmIndex) / TWO_PI : 0;
		let sum = 0;
		for (let i = 0; i < copies; i += 1) {
			const phase = (i / copies + t * ratios[i] + bend) % 1;
			sum += oscillator(shape, phase < 0 ? phase + 1 : phase, noise);
		}
		let value = copies > 1 ? sum / Math.sqrt(copies) : sum;
		if (sub > 0) value += sub * Math.sin(t * Math.PI);
		if (filter) value = filter(value);
		if (n >= warmup) values.push(value);
	}
	return normalize(values);
}

/**
 * `length` frames of `frames` starting at the first rising zero crossing that
 * leaves room for them, so a periodic wave stands still from read to read.
 * Without a crossing it falls back to the newest frames.
 */
export function triggerWindow(frames: number[], length: number): number[] {
	const span = Math.min(length, frames.length);
	const last = frames.length - span;
	for (let i = 1; i <= last; i += 1) {
		if (frames[i - 1] <= 0 && frames[i] > 0) return frames.slice(i, i + span);
	}
	return frames.slice(last);
}

/** SVG polyline points for `values` in [-1, 1] across a `width` by `height` box. */
export function wavePoints(values: number[], width: number, height: number): string {
	const step = values.length > 1 ? width / (values.length - 1) : 0;
	const middle = height / 2;
	const reach = middle * 0.9;
	return values.map((value, i) => `${i * step},${middle - clamp(value, -1, 1) * reach}`).join(' ');
}
