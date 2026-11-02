import { NoteCache } from './noteCache';
import type { NoteView, UiState } from './wire';

interface Matchers {
	toEqual(expected: unknown): void;
	toBe(expected: unknown): void;
	not: { toBe(expected: unknown): void };
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

const note = (pitch: number): NoteView => ({ pitch, startBeats: 0, lengthBeats: 1, velocity: 100 });

function snapshot(revisions: number[], notes: NoteView[][]): UiState {
	return {
		tracks: revisions.map((notesRevision, index) => ({
			notesRevision,
			blocks: [{ notes: notes[index] ?? [] }]
		}))
	} as unknown as UiState;
}

describe('NoteCache', () => {
	test('remembers what the engine sent and refills what it left out', () => {
		const cache = new NoteCache();
		expect(cache.known()).toEqual([]);
		cache.merge(snapshot([3, 4], [[note(60)], [note(62)]]));
		expect(cache.known()).toEqual([3, 4]);
		const idle = cache.merge(snapshot([3, 4], [[], []]));
		expect(idle.tracks.map((track) => track.blocks[0].notes)).toEqual([[note(60)], [note(62)]]);
	});

	test('takes the new notes when a revision moves', () => {
		const cache = new NoteCache();
		cache.merge(snapshot([3, 4], [[note(60)], [note(62)]]));
		const edited = cache.merge(snapshot([7, 4], [[note(64)], []]));
		expect(edited.tracks.map((track) => track.blocks[0].notes)).toEqual([[note(64)], [note(62)]]);
		expect(cache.known()).toEqual([7, 4]);
	});

	test('keeps the identity of unchanged tracks and replaces changed ones', () => {
		const cache = new NoteCache();
		const build = (level: number) =>
			({
				playing: false,
				tracks: [0, 1].map((index) => ({
					notesRevision: 1,
					level: index === 0 ? level : 0,
					blocks: [{ notes: [] }]
				}))
			}) as unknown as UiState;
		const first = cache.merge(build(0));
		const second = cache.merge(build(0.5));
		expect(second).not.toBe(first);
		expect(second.tracks[1]).toBe(first.tracks[1]);
		expect(second.tracks[1].blocks[0]).toBe(first.tracks[1].blocks[0]);
		expect(second.tracks[0]).not.toBe(first.tracks[0]);
		expect(cache.merge(build(0.5))).toBe(second);
	});
});
