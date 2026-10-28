import {
	MIN_BARS,
	MIN_LABEL_PX,
	ZOOM_DEFAULT,
	ZOOM_MAX,
	meterClips,
	meterPercent,
	ZOOM_MIN,
	anchoredScroll,
	barsFor,
	beatsToPx,
	clampRowHeight,
	clampZoom,
	envelopePoints,
	formatCountIn,
	formatPosition,
	formatPrecise,
	freeStart,
	gridStep,
	labelStep,
	position,
	pruneHeights,
	pxToBeats,
	rangePercent,
	snapBeats,
	stretchLength,
	trimSpan,
	waveWindow,
	waveformPoints,
	windowedWaveformPoints,
	zoomBy,
	zoomByWheel
} from './geometry';

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeNull(): void;
	toBeCloseTo(expected: number, digits?: number): void;
	toHaveLength(expected: number): void;
	toBeGreaterThan(expected: number): void;
	toBeLessThan(expected: number): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

describe('beatsToPx and pxToBeats', () => {
	test('scales a beat count by the pixels per beat', () => {
		expect(beatsToPx(4, 32)).toBe(128);
		expect(beatsToPx(0, 32)).toBe(0);
		expect(beatsToPx(1.5, 8)).toBe(12);
	});

	test('survives a round trip through the other direction', () => {
		for (const scale of [ZOOM_MIN, 32, 64, ZOOM_MAX]) {
			expect(pxToBeats(beatsToPx(7.5, scale), scale)).toBeCloseTo(7.5, 6);
			expect(pxToBeats(beatsToPx(0.25, scale), scale)).toBeCloseTo(0.25, 6);
		}
	});

	test('reads a pixel offset back as the beat under it', () => {
		expect(pxToBeats(128, 32)).toBe(4);
		expect(pxToBeats(0, 32)).toBe(0);
	});
});

describe('snapBeats', () => {
	test('rounds to the nearest beat', () => {
		expect(snapBeats(2.4, 1)).toBe(2);
		expect(snapBeats(2.6, 1)).toBe(3);
		expect(snapBeats(15.5, 1)).toBe(16);
	});

	test('leaves a beat already on the grid alone', () => {
		for (const beat of [0, 1, 2, 7, 64]) {
			expect(snapBeats(beat, 1)).toBe(beat);
		}
	});

	test('lands on a multiple of the snap grid', () => {
		for (const beat of [0.2, 1.1, 3.9, 12.6]) {
			const snapped = snapBeats(beat, 1);
			expect(snapped % 1).toBe(0);
		}
	});

	test('leaves the value alone when the drag is free', () => {
		expect(snapBeats(2.4, 1, true)).toBe(2.4);
		expect(snapBeats(2.6, 1, true)).toBe(2.6);
	});

	test('never returns a position above the top of the song', () => {
		expect(snapBeats(-3, 1)).toBe(0);
		expect(snapBeats(-3, 1, true)).toBe(0);
		expect(snapBeats(-0.4, 1)).toBe(0);
	});
});

describe('clampZoom', () => {
	test('keeps a scale inside the zoom range', () => {
		expect(clampZoom(ZOOM_DEFAULT)).toBe(ZOOM_DEFAULT);
		expect(clampZoom(ZOOM_MIN - 1)).toBe(ZOOM_MIN);
		expect(clampZoom(ZOOM_MAX + 1)).toBe(ZOOM_MAX);
	});
});

describe('zoomBy', () => {
	test('moves one step in the given direction', () => {
		expect(zoomBy(ZOOM_DEFAULT, 1)).toBe(40);
		expect(zoomBy(ZOOM_DEFAULT, -1)).toBeCloseTo(25.6, 6);
	});

	test('does not drift over a round trip', () => {
		expect(zoomBy(zoomBy(ZOOM_DEFAULT, 1), -1)).toBeCloseTo(ZOOM_DEFAULT, 6);
	});

	test('stops at the ends of the range', () => {
		expect(zoomBy(ZOOM_MAX, 1)).toBe(ZOOM_MAX);
		expect(zoomBy(ZOOM_MIN, -1)).toBe(ZOOM_MIN);
		expect(zoomBy(ZOOM_MAX * 4, 1)).toBe(ZOOM_MAX);
		expect(zoomBy(0, -1)).toBe(ZOOM_MIN);
	});
});

describe('position', () => {
	test('counts bars and beats from one', () => {
		expect(position(0, 4)).toEqual({ bar: 1, beat: 1, fraction: 0 });
		expect(position(1, 4)).toEqual({ bar: 1, beat: 2, fraction: 0 });
		expect(position(4, 4)).toEqual({ bar: 2, beat: 1, fraction: 0 });
		expect(position(16, 4)).toEqual({ bar: 5, beat: 1, fraction: 0 });
	});

	test('keeps the fraction of a beat inside the beat it belongs to', () => {
		expect(position(1.5, 4)).toEqual({ bar: 1, beat: 2, fraction: 0.5 });
		const late = position(7.9, 4);
		expect(late.bar).toBe(2);
		expect(late.beat).toBe(4);
		expect(late.fraction).toBeCloseTo(0.9, 6);
	});

	test('clamps a position above the top of the song', () => {
		expect(position(-4, 4)).toEqual({ bar: 1, beat: 1, fraction: 0 });
	});

	test('falls back to four beats a bar when the engine reports none', () => {
		expect(position(4, 0)).toEqual({ bar: 2, beat: 1, fraction: 0 });
	});
});

describe('formatCountIn', () => {
	test('shows the beats left rounded up, or nothing when no count-in runs', () => {
		expect(formatCountIn(4)).toBe('-4');
		expect(formatCountIn(1.2)).toBe('-2');
		expect(formatCountIn(0.01)).toBe('-1');
		expect(formatCountIn(0)).toBe(null);
	});
});

describe('formatPosition', () => {
	test('reads a beat as bar.beat', () => {
		expect(formatPosition(0, 4)).toBe('1.1');
		expect(formatPosition(5, 4)).toBe('2.2');
		expect(formatPosition(7.9, 4)).toBe('2.4');
	});

	test('follows the engine when the bar is not four beats', () => {
		expect(formatPosition(3, 3)).toBe('2.1');
		expect(formatPosition(2, 3)).toBe('1.3');
	});
});

describe('barsFor', () => {
	test('draws a minimum number of bars on an empty song', () => {
		expect(barsFor(0, 4)).toBe(MIN_BARS);
		expect(barsFor(4, 4)).toBe(MIN_BARS);
	});

	test('covers the material when the song is longer than the minimum', () => {
		expect(barsFor(32, 4)).toBe(8);
		expect(barsFor(33, 4)).toBe(9);
		expect(barsFor(64, 8)).toBe(8);
	});

	test('never draws fewer bars than the engine reported', () => {
		expect(barsFor(4, 4, 20)).toBe(20);
		expect(barsFor(200, 4, 12)).toBe(50);
	});

	test('falls back to four beats a bar when the engine reports none', () => {
		expect(barsFor(32, 0)).toBe(8);
	});
});

describe('labelStep', () => {
	test('numbers every bar when the bars are wide', () => {
		expect(labelStep(128)).toBe(1);
		expect(labelStep(MIN_LABEL_PX)).toBe(1);
		expect(labelStep(MIN_LABEL_PX / 2)).toBe(2);
	});

	test('skips bars as they narrow, in powers of two', () => {
		expect(labelStep(32)).toBe(2);
		expect(labelStep(16)).toBe(4);
		expect(labelStep(8)).toBe(8);
		expect(labelStep(4)).toBe(16);
	});

	test('stops widening rather than looping forever', () => {
		expect(labelStep(0)).toBe(64);
	});
});

describe('freeStart', () => {
	test('puts the first block at the top of the song', () => {
		expect(freeStart([])).toBe(0);
	});

	test('ends where the material ends', () => {
		expect(freeStart([{ startBeats: 0, lengthBeats: 4 }])).toBe(4);
		expect(freeStart([{ startBeats: 8, lengthBeats: 2 }])).toBe(10);
	});

	test('takes the furthest end, whatever order the blocks arrive in', () => {
		expect(
			freeStart([
				{ startBeats: 16, lengthBeats: 4 },
				{ startBeats: 0, lengthBeats: 8 }
			])
		).toBe(20);
	});
});

describe('waveformPoints', () => {
	test('returns nothing for an empty peak list', () => {
		expect(waveformPoints([], 40)).toBe('');
	});

	test('puts a silent peak on the centre line', () => {
		expect(waveformPoints([0], 40)).toBe('0,20 0,20');
	});

	test('puts a full peak against both edges', () => {
		expect(waveformPoints([1], 40)).toBe('0,0 0,40');
	});

	test('mirrors the peaks around the centre line', () => {
		expect(waveformPoints([0.5, 1], 40)).toBe('0,10 1,0 1,40 0,30');
	});

	test('clamps a peak the engine should never send', () => {
		expect(waveformPoints([-1, 2], 40)).toBe('0,20 1,0 1,40 0,20');
	});

	test('windows peaks to the visible slice', () => {
		const peaks = [0.1, 0.4, 0.8, 0.2, 0.05];
		const result = waveformPoints(peaks, 40, { x: 1, width: 2 });
		expect(result).toBe('1,12 2,4 3,16 3,24 2,36 1,28');
	});

	test('returns empty when window is completely outside peaks', () => {
		expect(waveformPoints([1, 1], 40, { x: 10, width: 5 })).toBe('');
		expect(waveformPoints([1, 1], 40, { x: -10, width: 2 })).toBe('');
	});

	test('decimates large visible window smoothly', () => {
		const peaks = Array.from({ length: 1000 }, (_, i) => (i % 2 === 0 ? 0.8 : 0.2));
		const result = windowedWaveformPoints(peaks, { x: 0, width: 1000 }, 40, 50);
		const points = result.split(' ');
		expect(points.length).toBe(200);
	});
});

describe('envelopePoints', () => {
	const WIDTH = 240;
	const HEIGHT = 40;
	const PLATEAU = 80;

	test('starts silent, peaks at the attack and ends silent', () => {
		const points = envelopePoints(0.5, 0.5, 0.5, 0.5, WIDTH, HEIGHT, PLATEAU);
		const [start, peak, , , end] = points.split(' ');
		expect(start).toBe('0,40');
		expect(peak.endsWith(',0')).toBe(true);
		expect(Number(end.split(',')[0])).toBeCloseTo(WIDTH, 6);
		expect(end.endsWith(',40')).toBe(true);
	});

	test('holds the sustain level across the plateau', () => {
		const points = envelopePoints(0.5, 0.5, 0.25, 0.5, WIDTH, HEIGHT, PLATEAU).split(' ');
		const held = points[2].split(',')[1];
		expect(Number(held)).toBeCloseTo(30, 6);
		expect(points[3].split(',')[1]).toBe(held);
	});

	test('gives the three times the width the plateau leaves', () => {
		const even = envelopePoints(1, 1, 1, 1, WIDTH, HEIGHT, PLATEAU).split(' ');
		const rise = Number(even[1].split(',')[0]);
		const decay = Number(even[2].split(',')[0]) - rise;
		const tail = Number(even[4].split(',')[0]) - Number(even[3].split(',')[0]);
		expect(rise).toBeCloseTo((WIDTH - PLATEAU) / 3, 6);
		expect(decay).toBeCloseTo(rise, 6);
		expect(tail).toBeCloseTo(rise, 6);
	});

	test('survives an envelope with no time in it at all', () => {
		const points = envelopePoints(0, 0, 0, 0, WIDTH, HEIGHT, PLATEAU).split(' ');
		expect(points).toHaveLength(5);
		expect(points.every((point) => point.includes(','))).toBe(true);
	});

	test('clamps a sustain the engine should never send', () => {
		const loud = envelopePoints(1, 1, 4, 1, WIDTH, HEIGHT, PLATEAU).split(' ');
		expect(Number(loud[2].split(',')[1])).toBe(0);
	});
});

describe('row heights', () => {
	test('clamps to the allowed range', () => {
		expect(clampRowHeight(1)).toBe(32);
		expect(clampRowHeight(9999)).toBe(300);
		expect(clampRowHeight(100)).toBe(100);
	});

	test('prunes heights of removed tracks', () => {
		expect(pruneHeights({ 0: 80, 2: 90, 5: 40 }, 3)).toEqual({ 0: 80, 2: 90 });
	});
});

describe('trimSpan', () => {
	test('moves the start and keeps the end', () => {
		expect(trimSpan(2, 8, 0)).toEqual({ startBeats: 2, lengthBeats: 6 });
	});

	test('stops one shortest block before the end', () => {
		expect(trimSpan(9, 8, 0)).toEqual({ startBeats: 7.75, lengthBeats: 0.25 });
	});

	test('never pulls the start back past the floor', () => {
		expect(trimSpan(1, 8, 3)).toEqual({ startBeats: 3, lengthBeats: 5 });
	});
});

describe('stretchLength', () => {
	test('follows the pointer inside the rate range', () => {
		expect(stretchLength(4, 1, 6)).toBe(6);
	});

	test('stops where the rate reaches its limits', () => {
		expect(stretchLength(4, 1, 0.5)).toBe(1);
		expect(stretchLength(4, 1, 40)).toBe(16);
		expect(stretchLength(4, 2, 40)).toBe(32);
	});
});

describe('waveWindow', () => {
	test('shows the whole file for a block that plays all of it', () => {
		expect(waveWindow(0, 4, 120, 1, 2, 257)).toEqual({ x: 0, width: 256 });
	});

	test('starts at the offset and covers the played seconds', () => {
		expect(waveWindow(1, 1, 120, 1, 2, 257)).toEqual({ x: 128, width: 64 });
	});

	test('covers twice the file time at double rate', () => {
		expect(waveWindow(0, 1, 120, 2, 2, 257)).toEqual({ x: 0, width: 128 });
	});
});

describe('formatPrecise', () => {
	test('adds hundredths of a beat to bar and beat', () => {
		expect(formatPrecise(5.25, 4)).toBe('2.2.25');
		expect(formatPrecise(0, 4)).toBe('1.1.00');
		expect(formatPrecise(0.999, 4)).toBe('1.1.99');
	});
});

describe('gridStep', () => {
	test('snaps to finer divisions as the zoom grows', () => {
		expect(gridStep(8)).toBe(2);
		expect(gridStep(16)).toBe(1);
		expect(gridStep(32)).toBe(0.5);
		expect(gridStep(64)).toBe(0.25);
		expect(gridStep(256)).toBe(1 / 16);
	});

	test('snaps to a quarter-beat grid', () => {
		expect(snapBeats(2.3, 0.25)).toBe(2.25);
	});
});

describe('wheel zoom', () => {
	test('zooms in on an upward wheel and out on a downward one', () => {
		expect(zoomByWheel(32, -100)).toBeGreaterThan(32);
		expect(zoomByWheel(32, 100)).toBeLessThan(32);
	});

	test('stays inside the zoom range', () => {
		expect(zoomByWheel(ZOOM_MAX, -10_000)).toBe(ZOOM_MAX);
		expect(zoomByWheel(ZOOM_MIN, 10_000)).toBe(ZOOM_MIN);
	});

	test('keeps the beat under the pointer in place', () => {
		const left = anchoredScroll(320, 100, 32, 64);
		expect((left + 100) / 64).toBe((320 + 100) / 32);
	});
});

describe('rangePercent', () => {
	test('places a value between its bounds', () => {
		expect(rangePercent(0.5, 0, 1)).toBe('50%');
		expect(rangePercent(0, -1, 1)).toBe('50%');
	});

	test('stays inside 0 to 100', () => {
		expect(rangePercent(5, 0, 1)).toBe('100%');
		expect(rangePercent(-5, 0, 1)).toBe('0%');
		expect(rangePercent(1, 1, 1)).toBe('0%');
	});
});

describe('meterPercent', () => {
	test('puts full scale at the top and -60 dB at the bottom', () => {
		expect(meterPercent(1)).toBe(100);
		expect(meterPercent(0)).toBe(0);
		expect(meterPercent(0.001)).toBe(0);
	});

	test('reads -6 dB near 90 percent', () => {
		expect(Math.round(meterPercent(0.5))).toBe(90);
	});

	test('clips only at full scale', () => {
		expect(meterClips(0.9)).toBe(false);
		expect(meterClips(1)).toBe(true);
	});
});
