import {
	beatUnder,
	blockMenu,
	clampToViewport,
	deviceMenu,
	laneMenu,
	metronomeMenu,
	markAt,
	selectionMenu,
	splitPoint,
	trackMenu,
	type MenuEntry,
	type MenuItem
} from './menu';
import type { BeatRange, BlockView, EngineEvent, TrackView } from './wire';

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeNull(): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void | Promise<void>) => void;
declare const expect: (actual: unknown) => Matchers;

function host() {
	const sent: EngineEvent[] = [];
	const opened: Array<[number, number]> = [];
	return {
		sent,
		opened,
		request(event: EngineEvent): Promise<boolean> {
			sent.push(event);
			return Promise.resolve(true);
		},
		openRoll(track: number, block: number): void {
			opened.push([track, block]);
		}
	};
}

function block(overrides: Partial<BlockView> = {}): BlockView {
	return {
		name: 'b',
		kind: 'audio',
		startBeats: 8,
		lengthBeats: 4,
		source: '/tmp/kick.wav',
		selected: true,
		notes: [],
		offsetSeconds: 0,
		rate: 1,
		...overrides
	};
}

function labels(entries: MenuEntry[]): string[] {
	return entries.flatMap((entry) => (entry.kind === 'item' ? [entry.label] : ['---']));
}

function find(entries: MenuEntry[], label: string): MenuItem {
	const found = entries.find(
		(entry): entry is MenuItem => entry.kind === 'item' && entry.label === label
	);
	if (found === undefined) throw new Error(`no ${label} item`);
	return found;
}

describe('block menu', () => {
	test('only a MIDI block offers the piano roll, only audio offers reset stretch', () => {
		const midi = labels(blockMenu(host(), 0, 0, block({ kind: 'midi', source: '' }), 9, 1));
		const audio = labels(blockMenu(host(), 0, 0, block(), 9, 1));
		expect(midi.includes('Open in piano roll')).toBe(true);
		expect(midi.includes('Reset stretch')).toBe(false);
		expect(audio.includes('Open in piano roll')).toBe(false);
		expect(audio.includes('Reset stretch')).toBe(true);
	});

	test('reset stretch is disabled at rate one and restores the unstretched length', () => {
		expect(find(blockMenu(host(), 0, 0, block(), 9, 1), 'Reset stretch').disabled).toBe(true);
		const fake = host();
		const reset = find(blockMenu(fake, 1, 2, block({ rate: 2 }), 9, 1), 'Reset stretch');
		expect(reset.disabled).toBe(false);
		reset.run();
		expect(fake.sent).toEqual([{ stretchBlock: { track: 1, block: 2, lengthBeats: 8 } }]);
	});

	test('split sends the snapped beat inside the block', () => {
		const fake = host();
		find(blockMenu(fake, 0, 3, block(), 9.4, 1), 'Split here').run();
		expect(fake.sent).toEqual([{ splitBlock: { track: 0, block: 3, atBeats: 9 } }]);
	});

	test('rename selects the block before it opens the prompt', async () => {
		const fake = host();
		find(blockMenu(fake, 0, 1, block(), 9, 1), 'Rename').run();
		await Promise.resolve();
		await Promise.resolve();
		expect(fake.sent).toEqual([{ selectBlock: { track: 0, block: 1 } }, 'renameBlock']);
	});
});

describe('split point', () => {
	test('the beat under the pointer counts from the block start', () => {
		expect(beatUnder(8, 64, 32)).toBe(10);
		expect(beatUnder(8, -5, 32)).toBe(8);
	});

	test('snaps to the grid when both halves stay long enough', () => {
		expect(splitPoint(block(), 10.4, 1)).toBe(10);
	});

	test('falls back to the raw beat when the grid line sits on an edge', () => {
		expect(splitPoint(block({ startBeats: 8, lengthBeats: 1 }), 8.4, 1)).toBe(8.4);
	});

	test('refuses a cut that would leave a sliver', () => {
		expect(splitPoint(block(), 8.1, 1)).toBeNull();
		expect(splitPoint(block(), 11.9, 1)).toBeNull();
	});
});

describe('lane and track menus', () => {
	test('an audio lane cannot create a block', () => {
		expect(find(laneMenu(host(), 0, 'audio', 3, 4, 1), 'New block here').disabled).toBe(true);
		expect(find(laneMenu(host(), 0, 'midi', 3, 4, 1), 'New block here').disabled).toBe(undefined);
	});

	test('loop from here spans one bar from the snapped beat', () => {
		const fake = host();
		find(laneMenu(fake, 0, 'midi', 5.2, 4, 1), 'Loop from here').run();
		expect(fake.sent).toEqual([{ setLoopRegion: { startBeats: 5, endBeats: 9 } }]);
	});

	test('mute flips only the mute toggle', () => {
		const calls: Array<[boolean, boolean]> = [];
		const track = { muted: false, solo: true } as TrackView;
		find(
			trackMenu(host(), 0, track, (muted, solo) => calls.push([muted, solo])),
			'Mute'
		).run();
		expect(calls).toEqual([[true, true]]);
	});
});

describe('viewport clamp', () => {
	test('leaves a menu that fits where it was opened', () => {
		expect(clampToViewport(100, 100, 150, 200, 1000, 800)).toEqual({
			x: 100,
			y: 100
		});
	});

	test('pulls a menu back from the right and bottom edges', () => {
		expect(clampToViewport(950, 700, 150, 200, 1000, 800)).toEqual({
			x: 846,
			y: 596
		});
	});

	test('pins a menu larger than the window to the top left margin', () => {
		expect(clampToViewport(50, 50, 2000, 2000, 1000, 800)).toEqual({
			x: 4,
			y: 4
		});
	});
});

describe('markAt', () => {
	test('marks the split point over a block', () => {
		expect(markAt(block(), 10.4, 1)).toBe(10);
		expect(markAt(block(), 10.4, 1, true)).toBe(10.4);
	});

	test('marks the grid line off every block', () => {
		expect(markAt(undefined, 3.6, 1)).toBe(4);
		expect(markAt(undefined, 3.6, 1, true)).toBe(3.6);
	});
});

describe('metronomeMenu', () => {
	test('lists the count-ins and the record grids under their headings', () => {
		const entries = metronomeMenu(host(), 1, 0);
		const headings = entries.flatMap((entry) => (entry.kind === 'heading' ? [entry.label] : []));
		expect(headings).toEqual(['Count-in', 'Record quantize']);
		const items = entries.flatMap((entry) => (entry.kind === 'item' ? [entry.label] : []));
		expect(items).toEqual(['None', '1 bar', '2 bars', '4 bars', 'Off', '1/4', '1/8', '1/16']);
	});

	test('checks the current count-in and grid', () => {
		const entries = metronomeMenu(host(), 2, 0.5);
		const checked = entries.flatMap((entry) => (entry.kind === 'item' && entry.checked ? [entry.label] : []));
		expect(checked).toEqual(['2 bars', '1/8']);
	});

	test('sends the count-in in bars and the grid in beats', () => {
		const target = host();
		const entries = metronomeMenu(target, 0, 0);
		find(entries, '4 bars').run();
		find(entries, '1/16').run();
		find(entries, 'Off').run();
		expect(target.sent).toEqual([
			{ setCountIn: { bars: 4 } },
			{ setRecordQuantize: { gridBeats: 0.25 } },
			{ setRecordQuantize: { gridBeats: 0 } }
		]);
	});
});

describe('deviceMenu', () => {
	test('moves a device left and right and deletes it', () => {
		const target = host();
		const entries = deviceMenu(target, 1, 1, 3, false, () => {});
		find(entries, 'Move left').run();
		find(entries, 'Move right').run();
		find(entries, 'Delete').run();
		expect(target.sent).toEqual([
			{ moveEffect: { track: 1, from: 1, to: 0 } },
			{ moveEffect: { track: 1, from: 1, to: 2 } },
			{ removeEffect: { track: 1, index: 1 } }
		]);
	});

	test('disables the moves at the ends of the chain', () => {
		const first = deviceMenu(host(), 0, 0, 2, false, () => {});
		const last = deviceMenu(host(), 0, 1, 2, false, () => {});
		expect(find(first, 'Move left').disabled).toBe(true);
		expect(find(first, 'Move right').disabled).toBe(false);
		expect(find(last, 'Move right').disabled).toBe(true);
	});

	test('offers Unfold on a folded device and runs the fold', () => {
		let folds = 0;
		const entries = deviceMenu(host(), 0, 0, 1, true, () => {
			folds += 1;
		});
		find(entries, 'Unfold').run();
		expect(folds).toBe(1);
	});
});


describe('selectionMenu', () => {
	const range: BeatRange = { firstTrack: 0, lastTrack: 1, startBeats: 4, endBeats: 8 };

	test('offers selection commands and dispatches their payloads', () => {
		const fake = host();
		let cleared = false;
		const entries = selectionMenu(
			fake,
			range,
			{ tracks: 2, lengthBeats: 4 },
			1,
			6,
			false,
			() => {
				cleared = true;
			}
		);
		find(entries, 'Cut').run();
		find(entries, 'Copy').run();
		find(entries, 'Paste').run();
		find(entries, 'Duplicate').run();
		find(entries, 'Delete').run();
		find(entries, 'Loop selection').run();
		find(entries, 'Clear selection').run();

		expect(fake.sent).toEqual([
			{ cutRange: { range } },
			{ copyRange: { range } },
			{ pasteRange: { track: 1, startBeats: 6 } },
			{ duplicateRange: { range } },
			{ deleteRange: { range } },
			{ setLoopRegion: { startBeats: 4, endBeats: 8 } },
			'toggleLoop'
		]);
		expect(cleared).toBe(true);
	});

	test('disables paste when clipboard is empty', () => {
		const entries = selectionMenu(host(), range, null, 0, 0, false, () => {});
		expect(find(entries, 'Paste').disabled).toBe(true);
	});

	test('toggles the loop label depending on loop state', () => {
		const looping = selectionMenu(host(), range, null, 0, 0, true, () => {});
		find(looping, 'Loop selection').run();
	});
});
