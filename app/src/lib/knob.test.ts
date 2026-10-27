import {
	BEAT_GRID,
	beatFraction,
	clampParam,
	dragParam,
	formatParam,
	fromNormal,
	speakParam,
	stepParam,
	toNormal,
	type DialSpec
} from './knob';

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeCloseTo(expected: number, digits?: number): void;
	toBeGreaterThan(expected: number): void;
	toBeLessThan(expected: number): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

function spec(unit: DialSpec['unit'], min: number, max: number, log = false, choices: string[] = []): DialSpec {
	return { unit, min, max, log, choices };
}

const GAIN = spec('decibels', -24, 12);
const CUTOFF = spec('hertz', 20, 20000, true);
const TIME = spec('beats', 0.125, 2);
const TYPE = spec('choice', 0, 2, false, ['Low-pass', 'High-pass', 'Band-pass']);

describe('normal position', () => {
	test('maps a linear range onto 0..1 and back', () => {
		expect(toNormal(-24, GAIN)).toBe(0);
		expect(toNormal(12, GAIN)).toBe(1);
		expect(toNormal(-6, GAIN)).toBeCloseTo(0.5);
		for (const value of [-24, -10, 0, 3.5, 12]) {
			expect(fromNormal(toNormal(value, GAIN), GAIN)).toBeCloseTo(value, 6);
		}
	});

	test('maps a log range so the geometric middle sits halfway', () => {
		expect(toNormal(Math.sqrt(20 * 20000), CUTOFF)).toBeCloseTo(0.5, 6);
		expect(fromNormal(0.5, CUTOFF)).toBeCloseTo(Math.sqrt(20 * 20000), 3);
		for (const value of [20, 100, 1000, 2000, 20000]) {
			expect(fromNormal(toNormal(value, CUTOFF), CUTOFF)).toBeCloseTo(value, 3);
		}
	});

	test('clamps values and positions outside the range', () => {
		expect(toNormal(100, GAIN)).toBe(1);
		expect(toNormal(-100, GAIN)).toBe(0);
		expect(fromNormal(2, CUTOFF)).toBeCloseTo(20000, 3);
		expect(fromNormal(-1, CUTOFF)).toBeCloseTo(20, 6);
		expect(clampParam(40, GAIN)).toBe(12);
	});

	test('keeps choices whole and beat times on the grid', () => {
		expect(clampParam(1.4, TYPE)).toBe(1);
		expect(clampParam(0.7, TIME)).toBe(0.6875);
		expect(fromNormal(0.5, TIME) / BEAT_GRID % 1).toBe(0);
	});
});

describe('drag and steps', () => {
	test('sweeps the whole range over 200 px of upward drag', () => {
		expect(dragParam(-24, GAIN, -200)).toBe(12);
		expect(dragParam(12, GAIN, 200)).toBe(-24);
		expect(dragParam(-6, GAIN, -100)).toBeCloseTo(12);
	});

	test('moves ten times finer with Shift', () => {
		expect(dragParam(-6, GAIN, -100, true)).toBeCloseTo(-6 + 1.8);
	});

	test('drags a log dial by position, not by hertz', () => {
		const up = dragParam(1000, CUTOFF, -20);
		const down = dragParam(1000, CUTOFF, 20);
		expect(up / 1000).toBeCloseTo(1000 / down, 6);
	});

	test('steps by one percent, a tenth with fine and ten with big, then clamps', () => {
		expect(stepParam(-6, GAIN, 1)).toBeCloseTo(-6 + 0.36);
		expect(stepParam(-6, GAIN, 1, 'fine')).toBeCloseTo(-6 + 0.036);
		expect(stepParam(-6, GAIN, -1, 'big')).toBeCloseTo(-6 - 3.6);
		expect(stepParam(12, GAIN, 1)).toBe(12);
		expect(stepParam(-24, GAIN, -1)).toBe(-24);
	});

	test('steps a choice one choice and a beat time one grid cell', () => {
		expect(stepParam(0, TYPE, 1)).toBe(1);
		expect(stepParam(2, TYPE, 1)).toBe(2);
		expect(stepParam(0.75, TIME, 1)).toBe(0.8125);
		expect(stepParam(0.75, TIME, -1, 'big')).toBe(0.5);
	});
});

describe('formatParam', () => {
	test('signs decibels and never shows a negative zero', () => {
		expect(formatParam(3, GAIN)).toBe('+3.0 dB');
		expect(formatParam(-12, GAIN)).toBe('-12.0 dB');
		expect(formatParam(-0.01, GAIN)).toBe('0.0 dB');
		expect(formatParam(0, GAIN)).toBe('0.0 dB');
	});

	test('reads hertz under 1 kHz and kilohertz above', () => {
		expect(formatParam(250, CUTOFF)).toBe('250 Hz');
		expect(formatParam(2500, CUTOFF)).toBe('2.5 kHz');
		expect(formatParam(12000, CUTOFF)).toBe('12 kHz');
		expect(formatParam(1250, CUTOFF)).toBe('1.25 kHz');
	});

	test('reads milliseconds with a decimal only under ten', () => {
		const ms = spec('milliseconds', 0.1, 1000, true);
		expect(formatParam(0.5, ms)).toBe('0.5 ms');
		expect(formatParam(10, ms)).toBe('10 ms');
		expect(formatParam(100.4, ms)).toBe('100 ms');
	});

	test('reads beats as a musical fraction when they are one', () => {
		expect(formatParam(0.125, TIME)).toBe('1/8');
		expect(formatParam(0.25, TIME)).toBe('1/4');
		expect(formatParam(0.75, TIME)).toBe('3/4');
		expect(formatParam(1, TIME)).toBe('1 beat');
		expect(formatParam(2, TIME)).toBe('2 beats');
		expect(formatParam(1.5, TIME)).toBe('1 1/2');
		expect(formatParam(0.7, TIME)).toBe('0.7 beats');
		expect(beatFraction(0.7)).toBe(null);
	});

	test('reads a ratio, a percent and a choice', () => {
		expect(formatParam(4, spec('ratio', 1, 20, true))).toBe('4:1');
		expect(formatParam(2.5, spec('ratio', 1, 20, true))).toBe('2.5:1');
		expect(formatParam(0.35, spec('percent', 0, 1))).toBe('35%');
		expect(formatParam(1, TYPE)).toBe('High-pass');
	});

	test('spells out the unit of a fraction for a screen reader', () => {
		expect(speakParam(0.75, TIME)).toBe('3/4 beat');
		expect(speakParam(1.5, TIME)).toBe('1 1/2 beats');
		expect(speakParam(1, TIME)).toBe('1 beat');
		expect(speakParam(3, GAIN)).toBe('+3.0 dB');
	});
});
