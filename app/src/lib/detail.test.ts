import {
	INITIAL,
	MIN_HEIGHT,
	clampHeight,
	openClip,
	restore,
	selectTrack,
	serialize,
	showView,
	toggleView,
	type DetailState
} from './detail';

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

const OPEN_DEVICES: DetailState = { open: true, view: 'devices', height: 300 };
const HIDDEN_DEVICES: DetailState = { open: false, view: 'devices', height: 300 };

describe('detail transitions', () => {
	test('opening a roll shows the Clip view and opens a hidden pane', () => {
		expect(openClip(OPEN_DEVICES)).toEqual({ ...OPEN_DEVICES, view: 'clip' });
		expect(openClip(HIDDEN_DEVICES)).toEqual({ open: true, view: 'clip', height: 300 });
	});

	test('selecting a track shows Devices only while the pane is open', () => {
		expect(selectTrack({ ...OPEN_DEVICES, view: 'clip' })).toEqual(OPEN_DEVICES);
		const hidden = { ...HIDDEN_DEVICES, view: 'clip' as const };
		expect(selectTrack(hidden)).toEqual(hidden);
	});

	test('Shift+Tab switches views while open and opens a hidden pane on its last view', () => {
		expect(toggleView(OPEN_DEVICES).view).toBe('clip');
		expect(toggleView(toggleView(OPEN_DEVICES))).toEqual(OPEN_DEVICES);
		expect(toggleView(HIDDEN_DEVICES)).toEqual(OPEN_DEVICES);
	});

	test('picking a view from the header opens the pane on it', () => {
		expect(showView(HIDDEN_DEVICES, 'clip')).toEqual({ open: true, view: 'clip', height: 300 });
	});
});

describe('clampHeight', () => {
	test('holds the height between 160 px and 60% of the window', () => {
		expect(clampHeight(100, 1000)).toBe(MIN_HEIGHT);
		expect(clampHeight(400, 1000)).toBe(400);
		expect(clampHeight(900, 1000)).toBe(600);
	});

	test('keeps the minimum in a window too short for it', () => {
		expect(clampHeight(300, 200)).toBe(MIN_HEIGHT);
	});
});

describe('saved state', () => {
	test('round trips the height and the open state', () => {
		expect(restore(serialize(HIDDEN_DEVICES))).toEqual({ ...INITIAL, open: false, height: 300 });
	});

	test('falls back to the initial state on nothing or garbage', () => {
		expect(restore(null)).toEqual(INITIAL);
		expect(restore('not json')).toEqual(INITIAL);
		expect(restore('42')).toEqual(INITIAL);
		expect(restore('{"open":"yes","height":"tall"}')).toEqual(INITIAL);
	});

	test('never restores a height under the minimum', () => {
		expect(restore('{"open":true,"height":12}').height).toBe(MIN_HEIGHT);
	});
});
