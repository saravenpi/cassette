import {
	biquadGainDb,
	computeCompositeCurve,
	formatFreq,
	freqToX,
	gainToY,
	isBandActive,
	unpackBands,
	xToFreq,
	yToGain
} from './eqEight';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeCloseTo(expected: number, precision?: number): void;
	toBeLessThan(expected: number): void;
	toHaveLength(expected: number): void;
	toContain(expected: string): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const it: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

describe('eqEight biquad responses', () => {
	it('evaluates flat response for 0 dB bell filter', () => {
		const gain = biquadGainDb(1000, 1000, 0, 1.0, 2);
		expect(Math.abs(gain)).toBeLessThan(1e-4);
	});

	it('boosts peak frequency with bell filter', () => {
		const gainAtCenter = biquadGainDb(1000, 1000, 6, 1.0, 2);
		expect(gainAtCenter).toBeCloseTo(6.0, 1);

		const gainFar = biquadGainDb(100, 1000, 6, 1.0, 2);
		expect(Math.abs(gainFar)).toBeLessThan(1.0);
	});

	it('attenuates low frequencies on low cut', () => {
		const gainSub = biquadGainDb(30, 200, 0, 0.71, 0);
		const gainHigh = biquadGainDb(2000, 200, 0, 0.71, 0);
		expect(gainSub).toBeLessThan(-20);
		expect(Math.abs(gainHigh)).toBeLessThan(0.5);
	});

	it('attenuates high frequencies on high cut', () => {
		const gainSub = biquadGainDb(100, 5000, 0, 0.71, 4);
		const gainHigh = biquadGainDb(15000, 5000, 0, 0.71, 4);
		expect(Math.abs(gainSub)).toBeLessThan(0.5);
		expect(gainHigh).toBeLessThan(-15);
	});

	it('notches center frequency', () => {
		const gainCenter = biquadGainDb(1000, 1000, 0, 5.0, 5);
		const gainFar = biquadGainDb(200, 1000, 0, 5.0, 5);
		expect(gainCenter).toBeLessThan(-30);
		expect(Math.abs(gainFar)).toBeLessThan(0.5);
	});
});

describe('eqEight coordinate transforms', () => {
	it('converts frequency to X and back', () => {
		const width = 600;
		const f1 = 20;
		const f2 = 1000;
		const f3 = 20000;

		expect(freqToX(f1, width)).toBeCloseTo(0, 1);
		expect(freqToX(f3, width)).toBeCloseTo(width, 1);

		const x = freqToX(f2, width);
		expect(xToFreq(x, width)).toBeCloseTo(f2, 1);
	});

	it('converts gain to Y and back', () => {
		const height = 200;
		expect(gainToY(24, height)).toBeCloseTo(0, 1);
		expect(gainToY(0, height)).toBeCloseTo(height / 2, 1);
		expect(gainToY(-24, height)).toBeCloseTo(height, 1);

		const y = gainToY(6, height);
		expect(yToGain(y, height)).toBeCloseTo(6, 1);
	});
});

describe('eqEight composite curve and unpacking', () => {
	it('unpacks default 8 bands and calculates composite curve', () => {
		const params = new Array(32).fill(0);
		params[0] = 30;
		params[2] = 0.71;
		params[3] = 0;
		const bands = unpackBands(params);
		expect(bands).toHaveLength(8);
		expect(bands[0].freq).toBe(30);
		expect(bands[0].type).toBe(0);

		const curve = computeCompositeCurve(bands, 500, 200);
		expect(curve.path).toContain('M ');
		expect(curve.area).toContain('Z');
	});

	it('detects band activity correctly', () => {
		expect(isBandActive({ freq: 1000, gain: 0, q: 1, type: 2 })).toBe(false);
		expect(isBandActive({ freq: 1000, gain: 3, q: 1, type: 2 })).toBe(true);
		expect(isBandActive({ freq: 1000, gain: 0, q: 0.71, type: 0 })).toBe(true);
		expect(isBandActive({ freq: 1000, gain: 0, q: 0.71, type: 4 })).toBe(true);
		expect(isBandActive({ freq: 1000, gain: 0, q: 5, type: 5 })).toBe(true);
	});

	it('formats frequency numbers cleanly', () => {
		expect(formatFreq(50)).toBe('50');
		expect(formatFreq(1000)).toBe('1k');
		expect(formatFreq(2500)).toBe('2.5k');
		expect(formatFreq(10000)).toBe('10k');
	});
});
