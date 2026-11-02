import {
	normalize,
	peakOf,
	triggerWindow,
	voiceShape,
	wavePoints,
	type VoiceParams
} from './voice';

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeCloseTo(expected: number, digits?: number): void;
	toBeGreaterThan(expected: number): void;
	toBeLessThan(expected: number): void;
	toBeLessThanOrEqual(expected: number): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

function voice(overrides: Partial<VoiceParams> = {}): VoiceParams {
	return {
		waveform: 'Sine',
		unison: 1,
		detune: 0,
		sub: 0,
		cutoff: 20000,
		resonance: 0,
		fmRatio: 1,
		fmIndex: 0,
		...overrides
	};
}

function largestStep(values: number[]): number {
	let step = 0;
	for (let i = 0; i < values.length; i += 1) {
		const previous = values[(i + values.length - 1) % values.length];
		step = Math.max(step, Math.abs(values[i] - previous));
	}
	return step;
}

describe('voiceShape', () => {
	test('a plain sine is one cycle of the reference sine', () => {
		const shape = voiceShape(voice(), 256);
		expect(shape.length).toBe(256);
		shape.forEach((value, i) => expect(value).toBeCloseTo(Math.sin((2 * Math.PI * i) / 256), 9));
	});

	test('a plain saw ramps from -1 across one cycle', () => {
		const shape = voiceShape(voice({ waveform: 'Saw' }), 128);
		shape.forEach((value, i) => expect(value).toBeCloseTo(2 * (i / 128) - 1, 9));
	});

	test('the pulse is high for a quarter of the cycle', () => {
		const shape = voiceShape(voice({ waveform: 'Pulse' }), 100);
		expect(shape.filter((value) => value > 0).length).toBe(25);
	});

	test('unison, sub and FM stay normalized to a peak of one', () => {
		for (const overrides of [
			{ waveform: 'Saw', unison: 7, detune: 60 },
			{ waveform: 'Square', sub: 1 },
			{ waveform: 'Sine', fmRatio: 2, fmIndex: 8 },
			{ waveform: 'Triangle', unison: 3, detune: 20, sub: 0.5, cutoff: 800, resonance: 0.9 }
		]) {
			const shape = voiceShape(voice(overrides));
			expect(peakOf(shape)).toBeCloseTo(1, 9);
		}
	});

	test('a sub oscillator draws two cycles of the main oscillator', () => {
		const shape = voiceShape(voice({ waveform: 'Saw', sub: 0.001 }), 200);
		const resets = shape.filter((value, i) => i > 0 && value < shape[i - 1] - 0.5).length;
		expect(resets).toBe(1);
	});

	test('a closed filter rounds off the saw edge', () => {
		const open = voiceShape(voice({ waveform: 'Saw' }));
		const closed = voiceShape(voice({ waveform: 'Saw', cutoff: 600 }));
		expect(largestStep(closed)).toBeLessThan(largestStep(open) / 4);
	});

	test('noise is a seeded sketch that draws the same every time', () => {
		const first = voiceShape(voice({ waveform: 'Noise' }), 64);
		expect(voiceShape(voice({ waveform: 'Noise' }), 64)).toEqual(first);
		expect(new Set(first).size).toBeGreaterThan(32);
	});
});

describe('scope helpers', () => {
	test('the trigger starts the window on a rising zero crossing', () => {
		const frames = Array.from({ length: 1024 }, (_, i) => Math.sin(i * 0.05 + 2));
		const window = triggerWindow(frames, 512);
		expect(window.length).toBe(512);
		expect(window[0]).toBeGreaterThan(0);
		expect(window[0]).toBeLessThan(0.06);
		expect(window[1]).toBeGreaterThan(window[0]);
	});

	test('without a crossing the trigger falls back to the newest frames', () => {
		const frames = Array.from({ length: 10 }, (_, i) => i + 1);
		expect(triggerWindow(frames, 4)).toEqual([7, 8, 9, 10]);
	});

	test('normalize leaves silence alone and scales a quiet wave up', () => {
		expect(normalize([0, 0])).toEqual([0, 0]);
		expect(normalize([0.25, -0.5])).toEqual([0.5, -1]);
	});

	test('wave points span the box with the centre line at rest', () => {
		expect(wavePoints([0, 1, -1], 100, 50)).toBe('0,25 50,2.5 100,47.5');
	});
});
