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

class FakeElement {
	closest(_selector: string): any {
		return null;
	}
}

if (typeof (globalThis as any).Element === 'undefined') {
	(globalThis as any).Element = FakeElement;
}

class FakeWindow {
	private listeners: Record<string, ((event: any) => void)[]> = {};

	addEventListener(type: string, handler: (event: any) => void): void {
		if (!this.listeners[type]) this.listeners[type] = [];
		this.listeners[type].push(handler);
	}

	removeEventListener(type: string, handler: (event: any) => void): void {
		if (!this.listeners[type]) return;
		this.listeners[type] = this.listeners[type].filter((h) => h !== handler);
	}

	dispatchEvent(event: any): boolean {
		const list = this.listeners[event.type] ?? [];
		for (const h of list) {
			h(event);
		}
		return !event.defaultPrevented;
	}
}

class FakeKeyboardEvent {
	type: string;
	key: string;
	code: string;
	repeat: boolean;
	shiftKey: boolean;
	ctrlKey: boolean;
	altKey: boolean;
	metaKey: boolean;
	target: any;
	defaultPrevented: boolean = false;

	constructor(type: string, init: any = {}) {
		this.type = type;
		this.key = init.key ?? '';
		this.code = init.code ?? init.key ?? '';
		this.repeat = init.repeat ?? false;
		this.shiftKey = init.shiftKey ?? false;
		this.ctrlKey = init.ctrlKey ?? false;
		this.altKey = init.altKey ?? false;
		this.metaKey = init.metaKey ?? false;
		this.target = init.target ?? null;
	}

	preventDefault(): void {
		this.defaultPrevented = true;
	}
}

const fakeWindow = new FakeWindow();
Object.defineProperty(globalThis, 'window', {
	value: fakeWindow,
	configurable: true,
	writable: true
});

const { attachKeyboard } = await import('./keyboard');

describe('keyboard mixer navigation', () => {
	test('ArrowRight and ArrowLeft trigger track selection in mixer view', () => {
		let nextCalled = 0;
		let prevCalled = 0;

		const host = {
			activeView: 'mixer' as const,
			renaming: false,
			octave: 0,
			dispatch: () => {},
			sendNow: () => {},
			setZoom: () => {},
			requestScrollStart: () => {},
			openSelectedRoll: () => {},
			closeRoll: () => {},
			splitSelected: () => {},
			splitAtPointer: () => {},
			duplicate: () => {},
			toggleDetail: () => {},
			quantizeSelected: () => {},
			saveProject: () => {},
			saveProjectAs: () => {},
			loadProjectFromDialog: () => {},
			copySelection: () => {},
			cutSelection: () => {},
			pasteSelection: () => {},
			delete: () => {},
			loopSelection: () => {},
			clearSelection: () => false,
			nudgeSelectedBlock: () => false,
			seekCursor: () => {},
			selectNextTrack: () => {
				nextCalled += 1;
			},
			selectPreviousTrack: () => {
				prevCalled += 1;
			}
		};

		const teardown = attachKeyboard(host as any);

		const rightEvent = new FakeKeyboardEvent('keydown', { key: 'ArrowRight' });
		fakeWindow.dispatchEvent(rightEvent);
		expect(nextCalled).toBe(1);
		expect(rightEvent.defaultPrevented).toBe(true);

		const leftEvent = new FakeKeyboardEvent('keydown', { key: 'ArrowLeft' });
		fakeWindow.dispatchEvent(leftEvent);
		expect(prevCalled).toBe(1);
		expect(leftEvent.defaultPrevented).toBe(true);

		teardown();
	});

	test('Arrow keys do not navigate tracks when inside text input', () => {
		let nextCalled = 0;
		let prevCalled = 0;

		const host = {
			activeView: 'mixer' as const,
			renaming: false,
			octave: 0,
			dispatch: () => {},
			sendNow: () => {},
			setZoom: () => {},
			requestScrollStart: () => {},
			openSelectedRoll: () => {},
			closeRoll: () => {},
			splitSelected: () => {},
			splitAtPointer: () => {},
			duplicate: () => {},
			toggleDetail: () => {},
			quantizeSelected: () => {},
			saveProject: () => {},
			saveProjectAs: () => {},
			loadProjectFromDialog: () => {},
			copySelection: () => {},
			cutSelection: () => {},
			pasteSelection: () => {},
			delete: () => {},
			loopSelection: () => {},
			clearSelection: () => false,
			nudgeSelectedBlock: () => false,
			seekCursor: () => {},
			selectNextTrack: () => {
				nextCalled += 1;
			},
			selectPreviousTrack: () => {
				prevCalled += 1;
			}
		};

		const teardown = attachKeyboard(host as any);

		class InputElement extends FakeElement {
			closest(selector: string): any {
				if (selector.includes('input:not([type="range"])')) return this;
				return null;
			}
		}

		const fakeInput = new InputElement();

		const event = new FakeKeyboardEvent('keydown', { key: 'ArrowRight', target: fakeInput });
		fakeWindow.dispatchEvent(event);

		expect(nextCalled).toBe(0);
		expect(prevCalled).toBe(0);

		teardown();
	});

	test('Arrow keys do not navigate tracks in timeline view', () => {
		let nextCalled = 0;
		let prevCalled = 0;

		const host = {
			activeView: 'timeline' as const,
			renaming: false,
			octave: 0,
			dispatch: () => {},
			sendNow: () => {},
			setZoom: () => {},
			requestScrollStart: () => {},
			openSelectedRoll: () => {},
			closeRoll: () => {},
			splitSelected: () => {},
			splitAtPointer: () => {},
			duplicate: () => {},
			toggleDetail: () => {},
			quantizeSelected: () => {},
			saveProject: () => {},
			saveProjectAs: () => {},
			loadProjectFromDialog: () => {},
			copySelection: () => {},
			cutSelection: () => {},
			pasteSelection: () => {},
			delete: () => {},
			loopSelection: () => {},
			clearSelection: () => false,
			nudgeSelectedBlock: () => false,
			seekCursor: () => {},
			selectNextTrack: () => {
				nextCalled += 1;
			},
			selectPreviousTrack: () => {
				prevCalled += 1;
			}
		};

		const teardown = attachKeyboard(host as any);

		const event = new FakeKeyboardEvent('keydown', { key: 'ArrowRight' });
		fakeWindow.dispatchEvent(event);

		expect(nextCalled).toBe(0);

		teardown();
	});
});

describe('session track navigation logic', () => {
	test('navigates next track to last track then to master (999)', async () => {
		const { session } = await import('./session.svelte');

		session.snapshot = {
			bpm: 120,
			playing: false,
			recording: false,
			metronome: false,
			countInBars: 0,
			countInBeatsLeft: 0,
			recordQuantizeBeats: 0,
			looping: false,
			loopRegion: null,
			transportPositionBeats: 0,
			lengthBeats: 16,
			bars: 4,
			beatsPerBar: 4,
			cursor: { track: 0, block: 0 },
			octave: 0,
			presets: [],
			effectCatalog: [],
			tracks: [
				{ name: 'Track 1' } as any,
				{ name: 'Track 2' } as any,
				{ name: 'Track 3' } as any
			],
			status: '',
			renaming: false,
			canUndo: false,
			canRedo: false,
			backend: '',
			backendNote: '',
			sampleRate: 44100,
			projectPath: null,
			projectDirty: false,
			clipboard: null
		};

		session.selectTrack(0);
		expect(session.selectedTrack).toBe(0);

		session.selectNextTrack();
		expect(session.selectedTrack).toBe(1);

		session.selectNextTrack();
		expect(session.selectedTrack).toBe(2);

		session.selectNextTrack();
		expect(session.selectedTrack).toBe(999);

		session.selectNextTrack();
		expect(session.selectedTrack).toBe(999);
	});

	test('navigates previous track from master (999) to last track then down to 0', async () => {
		const { session } = await import('./session.svelte');

		session.snapshot = {
			bpm: 120,
			playing: false,
			recording: false,
			metronome: false,
			countInBars: 0,
			countInBeatsLeft: 0,
			recordQuantizeBeats: 0,
			looping: false,
			loopRegion: null,
			transportPositionBeats: 0,
			lengthBeats: 16,
			bars: 4,
			beatsPerBar: 4,
			cursor: { track: 0, block: 0 },
			octave: 0,
			presets: [],
			effectCatalog: [],
			tracks: [
				{ name: 'Track 1' } as any,
				{ name: 'Track 2' } as any,
				{ name: 'Track 3' } as any
			],
			status: '',
			renaming: false,
			canUndo: false,
			canRedo: false,
			backend: '',
			backendNote: '',
			sampleRate: 44100,
			projectPath: null,
			projectDirty: false,
			clipboard: null
		};

		session.selectTrack(999);
		expect(session.selectedTrack).toBe(999);

		session.selectPreviousTrack();
		expect(session.selectedTrack).toBe(2);

		session.selectPreviousTrack();
		expect(session.selectedTrack).toBe(1);

		session.selectPreviousTrack();
		expect(session.selectedTrack).toBe(0);

		session.selectPreviousTrack();
		expect(session.selectedTrack).toBe(0);
	});
});

export {};
