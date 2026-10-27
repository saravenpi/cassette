import {
	beatInRange,
	blockOverlapsRange,
	extendRange,
	formatRangeLength,
	normalizeRange,
	rangeLength,
	rangeTracks,
	resizeRangeEnd,
	resizeRangeStart,
	trackInRange
} from './selection';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

describe('normalizeRange', () => {
	test('orders tracks and beats so first <= last and start <= end', () => {
		expect(normalizeRange(3, 1, 16, 4)).toEqual({
			firstTrack: 1,
			lastTrack: 3,
			startBeats: 4,
			endBeats: 16
		});
		expect(normalizeRange(0, 0, 0, 8)).toEqual({
			firstTrack: 0,
			lastTrack: 0,
			startBeats: 0,
			endBeats: 8
		});
	});

	test('clamps negative beats to zero', () => {
		expect(normalizeRange(0, 1, -4, 8)).toEqual({
			firstTrack: 0,
			lastTrack: 1,
			startBeats: 0,
			endBeats: 8
		});
	});
});

describe('rangeLength and rangeTracks', () => {
	test('computes duration and track count', () => {
		const range = { firstTrack: 1, lastTrack: 3, startBeats: 4, endBeats: 12 };
		expect(rangeLength(range)).toBe(8);
		expect(rangeTracks(range)).toBe(3);
	});
});

describe('extendRange', () => {
	test('creates a zero-width range when null', () => {
		expect(extendRange(null, 2, 8)).toEqual({
			firstTrack: 2,
			lastTrack: 2,
			startBeats: 8,
			endBeats: 8
		});
	});

	test('widens track bounds and beat bounds', () => {
		const initial = { firstTrack: 1, lastTrack: 2, startBeats: 4, endBeats: 8 };
		expect(extendRange(initial, 0, 2)).toEqual({
			firstTrack: 0,
			lastTrack: 2,
			startBeats: 2,
			endBeats: 8
		});
		expect(extendRange(initial, 3, 16)).toEqual({
			firstTrack: 1,
			lastTrack: 3,
			startBeats: 4,
			endBeats: 16
		});
	});
});

describe('hit testing', () => {
	const range = { firstTrack: 1, lastTrack: 2, startBeats: 4, endBeats: 8 };

	test('tests whether track falls in range', () => {
		expect(trackInRange(0, range)).toBe(false);
		expect(trackInRange(1, range)).toBe(true);
		expect(trackInRange(2, range)).toBe(true);
		expect(trackInRange(3, range)).toBe(false);
	});

	test('tests whether beat falls in range', () => {
		expect(beatInRange(3.9, range)).toBe(false);
		expect(beatInRange(4, range)).toBe(true);
		expect(beatInRange(6, range)).toBe(true);
		expect(beatInRange(8, range)).toBe(true);
		expect(beatInRange(8.1, range)).toBe(false);
	});

	test('tests whether block overlaps range', () => {
		expect(blockOverlapsRange(1, 0, 4, range)).toBe(false);
		expect(blockOverlapsRange(1, 2, 4, range)).toBe(true);
		expect(blockOverlapsRange(1, 5, 2, range)).toBe(true);
		expect(blockOverlapsRange(1, 7, 3, range)).toBe(true);
		expect(blockOverlapsRange(1, 8, 2, range)).toBe(false);
		expect(blockOverlapsRange(0, 5, 2, range)).toBe(false);
	});
});

describe('resizing range edges', () => {
	const range = { firstTrack: 1, lastTrack: 2, startBeats: 4, endBeats: 8 };

	test('adjusts start beat', () => {
		expect(resizeRangeStart(range, 2)).toEqual({
			firstTrack: 1,
			lastTrack: 2,
			startBeats: 2,
			endBeats: 8
		});
	});

	test('swaps edges when start is dragged past end', () => {
		expect(resizeRangeStart(range, 10)).toEqual({
			firstTrack: 1,
			lastTrack: 2,
			startBeats: 8,
			endBeats: 10
		});
	});

	test('adjusts end beat', () => {
		expect(resizeRangeEnd(range, 12)).toEqual({
			firstTrack: 1,
			lastTrack: 2,
			startBeats: 4,
			endBeats: 12
		});
	});

	test('swaps edges when end is dragged before start', () => {
		expect(resizeRangeEnd(range, 2)).toEqual({
			firstTrack: 1,
			lastTrack: 2,
			startBeats: 2,
			endBeats: 4
		});
	});
});

describe('formatRangeLength', () => {
	test('formats integer bars and beats', () => {
		expect(formatRangeLength(0, 4)).toBe('0.0');
		expect(formatRangeLength(4, 4)).toBe('1.0');
		expect(formatRangeLength(6, 4)).toBe('1.2');
		expect(formatRangeLength(9, 4)).toBe('2.1');
	});

	test('formats fractional beats', () => {
		expect(formatRangeLength(1.5, 4)).toBe('0.1.50');
	});
});
