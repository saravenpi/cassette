import { NoteCache } from './noteCache';
import type { NoteView, UiState } from './wire';

interface Matchers {
	toEqual(expected: unknown): void;
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
});
