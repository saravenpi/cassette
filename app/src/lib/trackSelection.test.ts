interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

if (typeof (globalThis as Record<string, unknown>).$state === 'undefined') {
	(globalThis as Record<string, unknown>).$state = <T>(value: T) => value;
}

const { TrackSelectionStore } = await import('./trackSelection.svelte');

describe('TrackSelectionStore', () => {
	test('initializes with track 0 selected', () => {
		const store = new TrackSelectionStore();
		expect(Array.from(store.selectedTracks)).toEqual([0]);
		expect(store.lastClicked).toBe(0);
		expect(store.isSelected(0)).toBe(true);
		expect(store.isSelected(1)).toBe(false);
	});

	test('selects single track by default', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(3);
		expect(Array.from(store.selectedTracks)).toEqual([3]);
		expect(store.lastClicked).toBe(3);
		expect(store.isSelected(3)).toBe(true);
		expect(store.isSelected(0)).toBe(false);
	});

	test('selects contiguous range with shift forward and backward', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(1);
		store.selectTrack(4, { shift: true });
		expect(Array.from(store.selectedTracks).sort((a, b) => a - b)).toEqual([1, 2, 3, 4]);
		expect(store.lastClicked).toBe(1);

		store.selectTrack(4);
		store.selectTrack(1, { shift: true });
		expect(Array.from(store.selectedTracks).sort((a, b) => a - b)).toEqual([1, 2, 3, 4]);
		expect(store.lastClicked).toBe(4);
	});

	test('clamps shift range when totalTracks is provided', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(2);
		store.selectTrack(6, { shift: true, totalTracks: 4 });
		expect(Array.from(store.selectedTracks).sort((a, b) => a - b)).toEqual([2, 3]);
	});

	test('toggles tracks with toggle option', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(1);
		store.selectTrack(3, { toggle: true });
		expect(Array.from(store.selectedTracks).sort((a, b) => a - b)).toEqual([1, 3]);
		expect(store.lastClicked).toBe(3);

		store.selectTrack(1, { toggle: true });
		expect(Array.from(store.selectedTracks)).toEqual([3]);
		expect(store.lastClicked).toBe(1);
	});

	test('leaves sole selected track selected when attempting to toggle off', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(2);
		store.selectTrack(2, { toggle: true });
		expect(Array.from(store.selectedTracks)).toEqual([2]);
		expect(store.lastClicked).toBe(2);
	});

	test('selectAll selects all tracks up to totalTracks', () => {
		const store = new TrackSelectionStore();
		store.selectAll(5);
		expect(Array.from(store.selectedTracks).sort((a, b) => a - b)).toEqual([0, 1, 2, 3, 4]);

		store.selectAll(0);
		expect(Array.from(store.selectedTracks).sort((a, b) => a - b)).toEqual([0, 1, 2, 3, 4]);
	});

	test('clear resets to specified or default track', () => {
		const store = new TrackSelectionStore();
		store.selectAll(4);
		store.clear(2);
		expect(Array.from(store.selectedTracks)).toEqual([2]);
		expect(store.lastClicked).toBe(2);

		store.clear();
		expect(Array.from(store.selectedTracks)).toEqual([0]);
		expect(store.lastClicked).toBe(0);
	});

	test('deleteTracks shifts surviving indices down', () => {
		const store = new TrackSelectionStore();
		store.selectAll(5);
		store.deleteTracks([1, 3]);
		expect(Array.from(store.selectedTracks).sort((a, b) => a - b)).toEqual([0, 1, 2]);
	});

	test('deleteTracks drops removed tracks and shifts survivors', () => {
		const store = new TrackSelectionStore();
		store.clear(1);
		store.selectTrack(3, { toggle: true });
		store.deleteTracks([1]);
		expect(Array.from(store.selectedTracks)).toEqual([2]);
	});

	test('deleteTracks updates lastClicked when earlier tracks are deleted', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(4);
		store.deleteTracks([0, 2]);
		expect(store.lastClicked).toBe(2);
	});

	test('deleteTracks handles empty array without changing selection', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(2);
		store.deleteTracks([]);
		expect(Array.from(store.selectedTracks)).toEqual([2]);
		expect(store.lastClicked).toBe(2);
	});

	test('deleteTracks empties set when all selected tracks are deleted', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(1);
		store.selectTrack(2, { toggle: true });
		store.deleteTracks([1, 2]);
		expect(Array.from(store.selectedTracks)).toEqual([]);
	});

	test('reorderTrack updates selection indices and lastClicked', () => {
		const store = new TrackSelectionStore();
		store.selectTrack(1);
		store.reorderTrack(1, 3);
		expect(Array.from(store.selectedTracks)).toEqual([3]);
		expect(store.lastClicked).toBe(3);

		store.selectTrack(2);
		store.reorderTrack(3, 1);
		expect(Array.from(store.selectedTracks)).toEqual([3]);
	});
});

export {};

