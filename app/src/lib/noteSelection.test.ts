import {
	addSelection,
	applyGroupMove,
	applyGroupResizeLeft,
	applyGroupResizeRight,
	arrowNudge,
	arrowResize,
	arrowTranspose,
	clampGroupMove,
	clampGroupResizeLeft,
	clampGroupResizeRight,
	copySelection,
	duplicateSelection,
	marqueeSelect,
	pasteClipboard,
	rangeSelection,
	remapAfterDelete,
	toggleNote
} from './noteSelection';
import { MIN_NOTE_BEATS, PITCH_MAX, PITCH_MIN } from './pianoRoll';
import type { NoteView } from './wire';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeNull(): void;
	toBeCloseTo(expected: number, digits?: number): void;
	toBeLessThan(expected: number): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

function note(pitch: number, startBeats: number, lengthBeats: number, velocity = 100): NoteView {
	return { pitch, startBeats, lengthBeats, velocity };
}

describe('marqueeSelect', () => {
	const notes = [
		note(60, 0, 1),
		note(64, 1, 1),
		note(67, 1.5, 0.5),
		note(72, 2, 2)
	];

	test('selects notes that intersect beat and pitch bounds', () => {
		const hits = marqueeSelect(notes, {
			startBeat: 0.5,
			endBeat: 1.6,
			startPitch: 60,
			endPitch: 65
		});
		expect(Array.from(hits).sort()).toEqual([0, 1]);
	});

	test('handles marquee dragged in reverse', () => {
		const hits = marqueeSelect(notes, {
			startBeat: 2.5,
			endBeat: 0.8,
			startPitch: 70,
			endPitch: 60
		});
		expect(Array.from(hits).sort()).toEqual([0, 1, 2]);
	});

	test('excludes notes outside the pitch range even if beat overlaps', () => {
		const hits = marqueeSelect(notes, {
			startBeat: 0,
			endBeat: 4,
			startPitch: 61,
			endPitch: 63
		});
		expect(Array.from(hits)).toEqual([]);
	});
});

describe('selection sets', () => {
	test('toggleNote adds when absent and removes when present', () => {
		const empty = new Set<number>();
		const withOne = toggleNote(empty, 3);
		expect(Array.from(withOne)).toEqual([3]);
		const withoutOne = toggleNote(withOne, 3);
		expect(Array.from(withoutOne)).toEqual([]);
	});

	test('addSelection unions two sets', () => {
		const initial = new Set([1, 2]);
		const next = addSelection(initial, [2, 3, 4]);
		expect(Array.from(next).sort()).toEqual([1, 2, 3, 4]);
	});

	test('rangeSelection produces contiguous indices', () => {
		expect(Array.from(rangeSelection(4, 3))).toEqual([4, 5, 6]);
		expect(Array.from(rangeSelection(0, 0))).toEqual([]);
	});
});

describe('remapAfterDelete', () => {
	test('clears selection when all selected notes are deleted', () => {
		const selection = new Set([1, 2]);
		const next = remapAfterDelete(selection, [1, 2]);
		expect(Array.from(next)).toEqual([]);
	});

	test('shifts surviving indices down by the number of deletions before them', () => {
		const selection = new Set([0, 2, 4]);
		const next = remapAfterDelete(selection, [1, 3]);
		expect(Array.from(next).sort()).toEqual([0, 1, 2]);
	});

	test('drops deleted indices and preserves survivors', () => {
		const selection = new Set([1, 3, 5]);
		const next = remapAfterDelete(selection, [0, 1, 4]);
		expect(Array.from(next).sort()).toEqual([1, 2]);
	});
});

describe('clampGroupMove', () => {
	const chord = [
		note(60, 1, 1),
		note(64, 1.5, 1),
		note(67, 2, 1)
	];

	test('preserves chord shape when hitting pitch ceiling', () => {
		const clamped = clampGroupMove(chord, [0, 1, 2], 100, 0, 8);
		expect(clamped.deltaPitch).toBe(PITCH_MAX - 67);
		expect(clamped.deltaBeats).toBe(0);
	});

	test('preserves chord shape when hitting pitch floor', () => {
		const clamped = clampGroupMove(chord, [0, 1, 2], -100, 0, 8);
		expect(clamped.deltaPitch).toBe(PITCH_MIN - 60);
		expect(clamped.deltaBeats).toBe(0);
	});

	test('preserves relative timing when hitting block start', () => {
		const clamped = clampGroupMove(chord, [0, 1, 2], 0, -5, 8);
		expect(clamped.deltaBeats).toBe(-1);
	});

	test('preserves relative timing when hitting block end', () => {
		const clamped = clampGroupMove(chord, [0, 1, 2], 0, 10, 4);
		expect(clamped.deltaBeats).toBe(1);
	});

	test('applyGroupMove applies clamped deltas to each selected note', () => {
		const edits = applyGroupMove(chord, [0, 2], 2, 1, 8);
		expect(edits).toEqual([
			{ note: 0, pitch: 62, startBeats: 2, lengthBeats: 1, velocity: 100 },
			{ note: 2, pitch: 69, startBeats: 3, lengthBeats: 1, velocity: 100 }
		]);
	});
});

describe('left-edge resize', () => {
	const notes = [
		note(60, 1, 2),
		note(64, 2, 1)
	];

	test('keeps end fixed while moving start', () => {
		const clamped = clampGroupResizeLeft(notes, [0, 1], 0.5);
		expect(clamped).toBe(0.5);
		const edits = applyGroupResizeLeft(notes, [0, 1], 0.5);
		expect(edits[0].startBeats).toBe(1.5);
		expect(edits[0].lengthBeats).toBe(1.5);
		expect(edits[1].startBeats).toBe(2.5);
		expect(edits[1].lengthBeats).toBe(0.5);
	});

	test('clamps when reaching the block start', () => {
		const clamped = clampGroupResizeLeft(notes, [0, 1], -3);
		expect(clamped).toBe(-1);
	});

	test('clamps when the shortest note hits minimum length', () => {
		const clamped = clampGroupResizeLeft(notes, [0, 1], 2);
		expect(clamped).toBe(1 - MIN_NOTE_BEATS);
	});
});

describe('right-edge resize', () => {
	const notes = [
		note(60, 1, 2),
		note(64, 2, 1)
	];

	test('applies delta across group keeping starts fixed', () => {
		const clamped = clampGroupResizeRight(notes, [0, 1], 0.5, 8);
		expect(clamped).toBe(0.5);
		const edits = applyGroupResizeRight(notes, [0, 1], 0.5, 8);
		expect(edits[0].startBeats).toBe(1);
		expect(edits[0].lengthBeats).toBe(2.5);
		expect(edits[1].startBeats).toBe(2);
		expect(edits[1].lengthBeats).toBe(1.5);
	});

	test('clamps when latest end reaches block end', () => {
		const clamped = clampGroupResizeRight(notes, [0, 1], 10, 4);
		expect(clamped).toBe(1);
	});

	test('clamps when shortest note reaches minimum length', () => {
		const clamped = clampGroupResizeRight(notes, [0, 1], -5, 8);
		expect(clamped).toBe(MIN_NOTE_BEATS - 1);
	});
});

describe('clipboard and duplicate', () => {
	const notes = [
		note(60, 1, 1),
		note(64, 2, 1)
	];

	test('copySelection offsets notes relative to earliest start', () => {
		const clip = copySelection(notes, [0, 1]);
		expect(clip).not.toBeNull();
		expect(clip?.spanBeats).toBe(2);
		expect(clip?.latestEnd).toBe(3);
		expect(clip?.notes).toEqual([
			{ pitch: 60, startOffset: 0, lengthBeats: 1, velocity: 100 },
			{ pitch: 64, startOffset: 1, lengthBeats: 1, velocity: 100 }
		]);
	});

	test('pasteClipboard places notes at target beat', () => {
		const clip = copySelection(notes, [0, 1])!;
		const pasted = pasteClipboard(clip, 4, 8);
		expect(pasted).toEqual([
			{ pitch: 60, startBeats: 4, lengthBeats: 1, velocity: 100 },
			{ pitch: 64, startBeats: 5, lengthBeats: 1, velocity: 100 }
		]);
	});

	test('pasteClipboard falls back to copied end when target is null', () => {
		const clip = copySelection(notes, [0, 1])!;
		const pasted = pasteClipboard(clip, null, 8);
		expect(pasted).toEqual([
			{ pitch: 60, startBeats: 3, lengthBeats: 1, velocity: 100 },
			{ pitch: 64, startBeats: 4, lengthBeats: 1, velocity: 100 }
		]);
	});

	test('duplicateSelection places copy right after last end', () => {
		const dupe = duplicateSelection(notes, [0, 1], 8);
		expect(dupe).toEqual([
			{ pitch: 60, startBeats: 3, lengthBeats: 1, velocity: 100 },
			{ pitch: 64, startBeats: 4, lengthBeats: 1, velocity: 100 }
		]);
	});
});

describe('arrow nudge and transpose', () => {
	const notes = [
		note(60, 1, 1),
		note(64, 2, 1)
	];

	test('arrowNudge shifts beat starts by step', () => {
		const edits = arrowNudge(notes, [0, 1], 0.25, 4);
		expect(edits[0].startBeats).toBe(1.25);
		expect(edits[1].startBeats).toBe(2.25);
	});

	test('arrowTranspose transposes pitches by semitones', () => {
		const edits = arrowTranspose(notes, [0, 1], 12);
		expect(edits[0].pitch).toBe(72);
		expect(edits[1].pitch).toBe(76);
	});

	test('arrowResize adjusts length from the right edge', () => {
		const edits = arrowResize(notes, [0, 1], 0.25, 4);
		expect(edits[0].lengthBeats).toBe(1.25);
		expect(edits[1].lengthBeats).toBe(1.25);
	});
});
