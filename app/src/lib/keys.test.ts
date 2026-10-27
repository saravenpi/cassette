import { intentFor, isAppKey, localFor, noteFor, pitchFor, repeats } from './keys';
import type { EngineEvent } from './wire';

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeNull(): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

const MIDDLE_C = 60;

const ROW: ReadonlyArray<readonly [string, string, number]> = [
	['KeyA', 'a', 0],
	['KeyW', 'w', 1],
	['KeyS', 's', 2],
	['KeyE', 'e', 3],
	['KeyD', 'd', 4],
	['KeyF', 'f', 5],
	['KeyT', 't', 6],
	['KeyG', 'g', 7],
	['KeyY', 'y', 8],
	['KeyH', 'h', 9],
	['KeyU', 'u', 10],
	['KeyJ', 'j', 11],
	['KeyK', 'k', 12]
];

const EVERY_KEY: ReadonlyArray<readonly [string, string]> = [
	['KeyA', 'a'],
	['KeyC', 'c'],
	['KeyK', 'k'],
	['KeyL', 'l'],
	['KeyM', 'm'],
	['KeyN', 'n'],
	['KeyO', 'o'],
	['KeyQ', 'q'],
	['KeyR', 'r'],
	['KeyX', 'x'],
	['KeyZ', 'z'],
	['Space', ' '],
	['Enter', 'Enter'],
	['Escape', 'Escape'],
	['Backspace', 'Backspace'],
	['Delete', 'Delete'],
	['Minus', '-'],
	['Equal', '='],
	['BracketLeft', '['],
	['BracketRight', ']'],
	['Comma', ','],
	['Period', '.'],
	['Home', 'Home'],
	['NumpadAdd', '+'],
	['NumpadSubtract', '-'],
	['Tab', 'Tab'],
	['F1', 'F1'],
	['Shift', 'Shift'],
	['Control', 'Control'],
	['ArrowUp', 'ArrowUp'],
	['ArrowDown', 'ArrowDown'],
	['ArrowLeft', 'ArrowLeft'],
	['ArrowRight', 'ArrowRight']
];

function press(code: string, character: string = code, repeat = false): KeyboardEvent {
	return { code, key: character, repeat } as KeyboardEvent;
}

function held(event: KeyboardEvent): KeyboardEvent {
	return { ...event, shiftKey: true } as KeyboardEvent;
}

function chord(event: KeyboardEvent, modifier: 'ctrlKey' | 'metaKey' = 'ctrlKey'): KeyboardEvent {
	return { ...event, [modifier]: true } as KeyboardEvent;
}

describe('localFor', () => {
	test('sends Home to the top of the song', () => {
		expect(localFor(press('Home'), false)).toBe('scrollStart');
	});

	test('puts the zoom pair on the shifted minus and equals keys', () => {
		expect(localFor(held(press('Equal', '+')), false)).toBe('zoomIn');
		expect(localFor(held(press('Minus', '_')), false)).toBe('zoomOut');
	});

	test('puts the zoom pair on the numeric keypad as well', () => {
		expect(localFor(press('NumpadAdd', '+'), false)).toBe('zoomIn');
		expect(localFor(press('NumpadSubtract', '-'), false)).toBe('zoomOut');
	});

	test('leaves the unshifted minus and equals keys to the volume', () => {
		expect(localFor(press('Minus', '-'), false)).toBeNull();
		expect(localFor(press('Equal', '='), false)).toBeNull();
	});

	test('opens the piano roll on P and closes it on Escape', () => {
		expect(localFor(press('KeyP', 'p'), false)).toBe('openRoll');
		expect(localFor(press('Escape'), false)).toBe('closeRoll');
	});

	test('returns null for anything else', () => {
		expect(localFor(press('KeyQ', 'q'), false)).toBeNull();
		expect(localFor(press('Space', ' '), false)).toBeNull();
		expect(localFor(press('F1'), false)).toBeNull();
		expect(localFor(press('Shift'), false)).toBeNull();
	});

	test('returns null for every key while renaming', () => {
		for (const [code, character] of EVERY_KEY) {
			expect(localFor(press(code, character), true)).toBeNull();
			expect(localFor(held(press(code, character)), true)).toBeNull();
		}
	});
});

describe('pitchFor', () => {
	test('maps KeyA to middle C at octave 0', () => {
		expect(pitchFor('KeyA', 0)).toBe(60);
	});

	test('maps KeyK one octave above middle C at octave 0', () => {
		expect(pitchFor('KeyK', 0)).toBe(72);
	});

	test('maps every key of the tracker row to its semitone offset from C', () => {
		for (const [code, , semitone] of ROW) {
			expect(pitchFor(code, 0)).toBe(MIDDLE_C + semitone);
		}
	});

	test('shifts the whole row by twelve semitones per octave', () => {
		for (const [code, , semitone] of ROW) {
			expect(pitchFor(code, 1)).toBe(MIDDLE_C + 12 + semitone);
			expect(pitchFor(code, -1)).toBe(MIDDLE_C - 12 + semitone);
			expect(pitchFor(code, 3)).toBe(MIDDLE_C + 36 + semitone);
		}
	});

	test('returns null for a key that is not in the piano row', () => {
		expect(pitchFor('KeyQ', 0)).toBeNull();
		expect(pitchFor('Space', 0)).toBeNull();
		expect(pitchFor('KeyL', 0)).toBeNull();
		expect(pitchFor('ArrowUp', 0)).toBeNull();
		expect(pitchFor('', 0)).toBeNull();
	});

	test('clamps a pitch above 127 to 127', () => {
		expect(pitchFor('KeyA', 20)).toBe(127);
		expect(pitchFor('KeyK', 20)).toBe(127);
		expect(pitchFor('KeyK', 5)).toBe(127);
	});

	test('clamps a pitch below 0 to 0', () => {
		expect(pitchFor('KeyA', -20)).toBe(0);
		expect(pitchFor('KeyK', -20)).toBe(0);
		expect(pitchFor('KeyW', -6)).toBe(0);
	});

	test('keeps the row ends just inside the MIDI range at octaves 5 and -5', () => {
		expect(pitchFor('KeyG', 5)).toBe(127);
		expect(pitchFor('KeyY', 5)).toBe(127);
		expect(pitchFor('KeyA', -5)).toBe(0);
		expect(pitchFor('KeyW', -5)).toBe(1);
	});
});

describe('intentFor while renaming', () => {
	test('turns Escape into cancelRename', () => {
		expect(intentFor(press('Escape'), true)).toBe('cancelRename');
	});

	test('turns Enter into renameBlock, not addBlock', () => {
		expect(intentFor(press('Enter'), true)).toBe('renameBlock');
		expect(intentFor(press('KeyN', 'n'), true)).toEqual({ textInput: 'n' });
	});

	test('turns Backspace into textBackspace, not deleteBlock', () => {
		expect(intentFor(press('Backspace'), true)).toBe('textBackspace');
		expect(intentFor(press('Delete'), true)).toBeNull();
	});

	test('turns a single character key into textInput', () => {
		expect(intentFor(press('KeyA', 'a'), true)).toEqual({ textInput: 'a' });
		expect(intentFor(press('KeyZ', 'z'), true)).toEqual({ textInput: 'z' });
		expect(intentFor(press('Space', ' '), true)).toEqual({ textInput: ' ' });
		expect(intentFor(press('Period', '.'), true)).toEqual({ textInput: '.' });
	});

	test('turns every piano key into text instead of a note', () => {
		for (const [code, character] of ROW) {
			expect(intentFor(press(code, character), true)).toEqual({ textInput: character });
		}
	});

	test('turns Space into text rather than togglePlay', () => {
		expect(intentFor(press('Space', ' '), true)).not.toBe('togglePlay');
		expect(intentFor(press('Space', ' '), true)).toEqual({ textInput: ' ' });
	});

	test('returns null for a non printing key such as Shift', () => {
		expect(intentFor(press('Shift'), true)).toBeNull();
		expect(intentFor(press('Control'), true)).toBeNull();
		expect(intentFor(press('F1'), true)).toBeNull();
	});

	test('dispatches on event.key, not on event.code', () => {
		expect(intentFor(press('KeyA', 'q'), true)).toEqual({ textInput: 'q' });
		expect(intentFor(press('KeyQ', 'a'), true)).toEqual({ textInput: 'a' });
	});

	test('does not run the arrow or delta commands while renaming', () => {
		expect(intentFor(press('ArrowUp'), true)).toBeNull();
		expect(intentFor(press('ArrowDown'), true)).toBeNull();
		expect(intentFor(press('ArrowLeft'), true)).toBeNull();
		expect(intentFor(press('ArrowRight'), true)).toBeNull();
		expect(intentFor(press('Minus'), true)).toBeNull();
		expect(intentFor(press('KeyM', 'm'), true)).toEqual({ textInput: 'm' });
	});
});

describe('intentFor when not renaming', () => {
	test('turns Space into togglePlay', () => {
		expect(intentFor(press('Space', ' '), false)).toBe('togglePlay');
	});

	test('dispatches on event.code, not on event.key', () => {
		expect(intentFor(press('Space', 'q'), false)).toBe('togglePlay');
		expect(intentFor(press('KeyQ', ' '), false)).toBeNull();
	});

	test('turns the arrow keys into track and block moves', () => {
		expect(intentFor(press('ArrowUp'), false)).toEqual({ moveTrack: -1 });
		expect(intentFor(press('ArrowDown'), false)).toEqual({ moveTrack: 1 });
		expect(intentFor(press('ArrowLeft'), false)).toEqual({ moveBlock: -1 });
		expect(intentFor(press('ArrowRight'), false)).toEqual({ moveBlock: 1 });
	});

	test('turns KeyZ and KeyX into octave shifts', () => {
		expect(intentFor(press('KeyZ', 'z'), false)).toEqual({ octaveDelta: -1 });
		expect(intentFor(press('KeyX', 'x'), false)).toEqual({ octaveDelta: 1 });
	});

	test('turns the volume, pan and bpm keys into deltas', () => {
		expect(intentFor(press('Minus', '-'), false)).toEqual({ volumeDelta: -0.05 });
		expect(intentFor(press('Equal', '='), false)).toEqual({ volumeDelta: 0.05 });
		expect(intentFor(press('BracketLeft', '['), false)).toEqual({ panDelta: -0.1 });
		expect(intentFor(press('BracketRight', ']'), false)).toEqual({ panDelta: 0.1 });
		expect(intentFor(press('Comma', ','), false)).toEqual({ bpmDelta: -1 });
		expect(intentFor(press('Period', '.'), false)).toEqual({ bpmDelta: 1 });
	});

	test('turns the block, loop, kind, mute and solo keys into their commands', () => {
		expect(intentFor(press('Enter'), false)).toBe('addBlock');
		expect(intentFor(press('KeyN', 'n'), false)).toBe('addBlock');
		expect(intentFor(press('Backspace'), false)).toBe('deleteBlock');
		expect(intentFor(press('Delete'), false)).toBe('deleteBlock');
		expect(intentFor(press('KeyL', 'l'), false)).toBe('toggleLoop');
		expect(intentFor(press('KeyC', 'c'), false)).toBe('cycleTrackKind');
		expect(intentFor(press('KeyR', 'r'), false)).toBe('renameBlock');
		expect(intentFor(press('KeyM', 'm'), false)).toBe('toggleMute');
		expect(intentFor(press('KeyO', 'o'), false)).toBe('toggleSolo');
	});

	test('returns null for an unbound key such as KeyQ', () => {
		expect(intentFor(press('KeyQ', 'q'), false)).toBeNull();
		expect(intentFor(press('KeyP', 'p'), false)).toBeNull();
		expect(intentFor(press('Tab'), false)).toBeNull();
		expect(intentFor(press('F1'), false)).toBeNull();
	});

	test('leaves the zoom and scroll keys to the interface', () => {
		expect(intentFor(press('Home'), false)).toBeNull();
		expect(intentFor(press('NumpadAdd', '+'), false)).toBeNull();
		expect(intentFor(press('NumpadSubtract', '-'), false)).toBeNull();
		expect(intentFor(held(press('Equal', '+')), false)).toBeNull();
		expect(intentFor(held(press('Minus', '_')), false)).toBeNull();
	});

	test('leaves Escape to the interface, which closes the piano roll with it', () => {
		expect(intentFor(press('Escape'), false)).toBeNull();
	});

	test('never turns a piano key into a command', () => {
		for (const [code, character] of ROW) {
			expect(intentFor(press(code, character), false)).toBeNull();
		}
	});
});

describe('isAppKey', () => {
	test('returns true for Space', () => {
		expect(isAppKey(press('Space', ' '), false)).toBe(true);
	});

	test('returns true for every piano key', () => {
		for (const [code, character] of ROW) {
			expect(isAppKey(press(code, character), false)).toBe(true);
		}
	});

	test('returns true for every key while renaming', () => {
		for (const [code, character] of EVERY_KEY) {
			expect(isAppKey(press(code, character), true)).toBe(true);
		}
	});

	test('returns false for a key the app does not own, such as F1 or Tab', () => {
		expect(isAppKey(press('F1'), false)).toBe(false);
		expect(isAppKey(press('Tab'), false)).toBe(false);
		expect(isAppKey(press('Shift'), false)).toBe(false);
	});

	test('returns false for an unbound letter such as KeyQ', () => {
		expect(isAppKey(press('KeyQ', 'q'), false)).toBe(false);
		expect(isAppKey(press('KeyV', 'v'), false)).toBe(false);
	});

	test('returns true for the zoom, scroll and piano roll keys the interface owns', () => {
		expect(isAppKey(press('Home'), false)).toBe(true);
		expect(isAppKey(press('KeyP', 'p'), false)).toBe(true);
		expect(isAppKey(press('Escape'), false)).toBe(true);
		expect(isAppKey(press('NumpadAdd', '+'), false)).toBe(true);
		expect(isAppKey(press('NumpadSubtract', '-'), false)).toBe(true);
		expect(isAppKey(held(press('Equal', '+')), false)).toBe(true);
		expect(isAppKey(held(press('Minus', '_')), false)).toBe(true);
	});
});

describe('repeats', () => {
	const ONCE_PER_PRESS: EngineEvent[] = [
		'togglePlay',
		'stop',
		'toggleLoop',
		'addBlock',
		'deleteBlock',
		'cycleTrackKind',
		'renameBlock',
		'cancelRename',
		'toggleMute',
		'toggleSolo'
	];

	test('returns false for every intent that fires once per press', () => {
		for (const intent of ONCE_PER_PRESS) {
			expect(repeats(intent)).toBe(false);
		}
	});

	test('returns true for movement intents so they keep moving while held', () => {
		expect(repeats({ moveTrack: -1 })).toBe(true);
		expect(repeats({ moveTrack: 1 })).toBe(true);
		expect(repeats({ moveBlock: -1 })).toBe(true);
		expect(repeats({ moveBlock: 1 })).toBe(true);
	});

	test('returns true for the volume, pan, bpm and octave deltas', () => {
		expect(repeats({ volumeDelta: 0.05 })).toBe(true);
		expect(repeats({ volumeDelta: -0.05 })).toBe(true);
		expect(repeats({ panDelta: 0.1 })).toBe(true);
		expect(repeats({ bpmDelta: 1 })).toBe(true);
		expect(repeats({ octaveDelta: -1 })).toBe(true);
	});

	test('returns true for text and note intents', () => {
		expect(repeats({ textInput: 'a' })).toBe(true);
		expect(repeats('textBackspace')).toBe(true);
		expect(repeats({ noteOn: { pitch: 60, velocity: 1 } })).toBe(true);
		expect(repeats({ noteOff: { pitch: 60 } })).toBe(true);
	});

	test('returns true for a string intent outside the once per press set', () => {
		expect(repeats('notACommand' as EngineEvent)).toBe(true);
	});
});

describe('keymap invariant', () => {
	test('binds no key to both a note and a command', () => {
		for (const [code, character] of EVERY_KEY) {
			const isCommand = intentFor(press(code, character), false) !== null;
			if (isCommand) expect(pitchFor(code, 0)).toBeNull();
		}
	});
});

describe('clip editing keys', () => {
	test('splits at the pointer on B and at the playhead on Ctrl or Cmd plus E', () => {
		expect(localFor(press('KeyB', 'b'), false)).toBe('splitAtPointer');
		expect(localFor(chord(press('KeyE', 'e')), false)).toBe('splitAtPlayhead');
		expect(localFor(chord(press('KeyE', 'e'), 'metaKey'), false)).toBe('splitAtPlayhead');
	});

	test('duplicates on Ctrl or Cmd plus D', () => {
		expect(localFor(chord(press('KeyD', 'd')), false)).toBe('duplicate');
		expect(localFor(chord(press('KeyD', 'd'), 'metaKey'), false)).toBe('duplicate');
	});

	test('plays no note on a chord, and still plays the plain key', () => {
		expect(noteFor(chord(press('KeyE', 'e')), 0)).toBeNull();
		expect(noteFor(chord(press('KeyD', 'd')), 0)).toBeNull();
		expect(noteFor(press('KeyE', 'e'), 0)).toBe(63);
	});

	test('leaves other chords to the webview', () => {
		expect(intentFor(chord(press('KeyQ', 'q')), false)).toBeNull();
		expect(localFor(chord(press('KeyQ', 'q')), false)).toBeNull();
		expect(isAppKey(chord(press('KeyQ', 'q')), false)).toBe(false);
		expect(isAppKey(chord(press('KeyD', 'd')), false)).toBe(true);
	});

	test('maps arrangement editing chords: copy, cut, paste, loop', () => {
		expect(localFor(chord(press('KeyC', 'c')), false)).toBe('copy');
		expect(localFor(chord(press('KeyC', 'c'), 'metaKey'), false)).toBe('copy');
		expect(localFor(chord(press('KeyX', 'x')), false)).toBe('cut');
		expect(localFor(chord(press('KeyX', 'x'), 'metaKey'), false)).toBe('cut');
		expect(localFor(chord(press('KeyV', 'v')), false)).toBe('paste');
		expect(localFor(chord(press('KeyV', 'v'), 'metaKey'), false)).toBe('paste');
		expect(localFor(chord(press('KeyL', 'l')), false)).toBe('loopSelection');
		expect(localFor(chord(press('KeyL', 'l'), 'metaKey'), false)).toBe('loopSelection');
	});

	test('maps project chords: save, save as, open', () => {
		expect(localFor(chord(press('KeyS', 's')), false)).toBe('save');
		expect(localFor(chord(press('KeyS', 's'), 'metaKey'), false)).toBe('save');
		expect(localFor(held(chord(press('KeyS', 'S'))), false)).toBe('saveAs');
		expect(localFor(held(chord(press('KeyS', 'S'), 'metaKey')), false)).toBe('saveAs');
		expect(localFor(chord(press('KeyO', 'o')), false)).toBe('open');
		expect(localFor(chord(press('KeyO', 'o'), 'metaKey'), false)).toBe('open');
	});

	test('maps track add chords: Ctrl/Cmd+T and Ctrl/Cmd+Shift+T', () => {
		expect(intentFor(chord(press('KeyT', 't')), false)).toEqual({ addTrack: { kind: 'audio' } });
		expect(intentFor(chord(press('KeyT', 't'), 'metaKey'), false)).toEqual({
			addTrack: { kind: 'audio' }
		});
		expect(intentFor(held(chord(press('KeyT', 'T'))), false)).toEqual({
			addTrack: { kind: 'midi' }
		});
		expect(intentFor(held(chord(press('KeyT', 'T'), 'metaKey')), false)).toEqual({
			addTrack: { kind: 'midi' }
		});
		expect(isAppKey(chord(press('KeyT', 't')), false)).toBe(true);
	});

	test('undoes on Ctrl+Z and redoes on Ctrl+Shift+Z or Ctrl+Y', () => {
		expect(intentFor(chord(press('KeyZ', 'z')), false)).toBe('undo');
		expect(intentFor(chord(press('KeyZ', 'z'), 'metaKey'), false)).toBe('undo');
		expect(intentFor(held(chord(press('KeyZ', 'Z'))), false)).toBe('redo');
		expect(intentFor(chord(press('KeyY', 'y')), false)).toBe('redo');
		expect(isAppKey(chord(press('KeyZ', 'z')), false)).toBe(true);
		expect(intentFor(press('KeyZ', 'z'), false)).toEqual({ octaveDelta: -1 });
	});

	test('types no letter for a chord while renaming', () => {
		expect(intentFor(chord(press('KeyZ', 'z')), true)).toBeNull();
	});

	test('treats AltGr as typing, not as a chord', () => {
		const altGr = {
			...chord(press('KeyE', '€')),
			altKey: true,
			getModifierState: (key: string) => key === 'AltGraph'
		} as KeyboardEvent;
		expect(localFor(altGr, false)).toBeNull();
	});

	test('types B into a name instead of splitting', () => {
		expect(localFor(press('KeyB', 'b'), true)).toBeNull();
		expect(intentFor(press('KeyB', 'b'), true)).toEqual({ textInput: 'b' });
	});
});

describe('detail pane and quantize keys', () => {
	test('switches the detail view on Shift+Tab and leaves plain Tab to focus', () => {
		expect(localFor(held(press('Tab')), false)).toBe('toggleDetail');
		expect(localFor(press('Tab'), false)).toBeNull();
		expect(isAppKey(held(press('Tab')), false)).toBe(true);
		expect(intentFor(held(press('Tab')), false)).toBeNull();
	});

	test('quantizes on Ctrl or Cmd plus U, and U alone still plays its note', () => {
		expect(localFor(chord(press('KeyU', 'u')), false)).toBe('quantize');
		expect(localFor(chord(press('KeyU', 'u'), 'metaKey'), false)).toBe('quantize');
		expect(noteFor(chord(press('KeyU', 'u')), 0)).toBeNull();
		expect(noteFor(press('KeyU', 'u'), 0)).toBe(70);
		expect(isAppKey(chord(press('KeyU', 'u')), false)).toBe(true);
	});

	test('does neither while a name is being typed', () => {
		expect(localFor(held(press('Tab')), true)).toBeNull();
		expect(localFor(chord(press('KeyU', 'u')), true)).toBeNull();
	});

	test('binds no interface key to a note', () => {
		for (const [code, character] of EVERY_KEY) {
			const plain = localFor(press(code, character), false);
			const shifted = localFor(held(press(code, character)), false);
			if (plain !== null || shifted !== null) expect(pitchFor(code, 0)).toBeNull();
		}
	});
});
