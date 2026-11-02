import { groupMove, selectedBlocks } from './blockSelection';
import type { BlockView, TrackKind, TrackView } from './wire';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

function block(kind: TrackKind, startBeats: number, selected: boolean): BlockView {
	return {
		name: String(startBeats),
		kind,
		startBeats,
		lengthBeats: 4,
		source: '',
		selected,
		notes: [],
		offsetSeconds: 0,
		rate: 1
	};
}

function track(kind: TrackKind, blocks: BlockView[]): TrackView {
	return { kind, blocks } as unknown as TrackView;
}

const tracks = [
	track('midi', [block('midi', 0, true), block('midi', 8, false)]),
	track('midi', [block('midi', 4, true)]),
	track('audio', [block('audio', 2, false)])
];

describe('selectedBlocks', () => {
	test('lists the selected blocks in track then lane order', () => {
		const picked = selectedBlocks(tracks).map(({ track, block }) => [track, block]);
		expect(picked).toEqual([
			[0, 0],
			[1, 0]
		]);
	});
});

describe('groupMove', () => {
	const members = selectedBlocks(tracks);

	test('moves by the grabbed block and stops at the top of the song', () => {
		expect(groupMove(members, tracks, 4, 10, 0)).toEqual({
			deltaBeats: 6,
			deltaTracks: 0,
			invalid: false
		});
		expect(groupMove(members, tracks, 4, -20, 0).deltaBeats).toBe(0);
	});

	test('refuses a move off the tracks or onto the other kind', () => {
		expect(groupMove(members, tracks, 4, 4, -1).invalid).toBe(true);
		expect(groupMove(members, tracks, 4, 4, 1).invalid).toBe(true);
		expect(groupMove(members.slice(0, 1), tracks, 0, 0, 1).invalid).toBe(false);
	});
});
