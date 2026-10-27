/**
 * The webview keymap: the piano row, the engine commands, and the zoom, scroll
 * and piano roll keys that only the interface itself handles.
 */

import type { EngineEvent } from './wire';

/** Semitone offsets from C, in the tracker layout most DAW keyboards already use. */
const PIANO: Record<string, number> = {
	KeyA: 0,
	KeyW: 1,
	KeyS: 2,
	KeyE: 3,
	KeyD: 4,
	KeyF: 5,
	KeyT: 6,
	KeyG: 7,
	KeyY: 8,
	KeyH: 9,
	KeyU: 10,
	KeyJ: 11,
	KeyK: 12
};

/** Commands live on keys the piano row does not use (A W S E D F T G Y H U J K). */
const COMMANDS: Record<string, EngineEvent> = {
	Space: 'togglePlay',
	KeyL: 'toggleLoop',
	ArrowUp: { moveTrack: -1 },
	ArrowDown: { moveTrack: 1 },
	ArrowLeft: { moveBlock: -1 },
	ArrowRight: { moveBlock: 1 },
	Enter: 'addBlock',
	KeyN: 'addBlock',
	Backspace: 'deleteBlock',
	Delete: 'deleteBlock',
	KeyC: 'cycleTrackKind',
	KeyR: 'renameBlock',
	KeyM: 'toggleMute',
	KeyO: 'toggleSolo',
	Minus: { volumeDelta: -0.05 },
	Equal: { volumeDelta: 0.05 },
	BracketLeft: { panDelta: -0.1 },
	BracketRight: { panDelta: 0.1 },
	Comma: { bpmDelta: -1 },
	Period: { bpmDelta: 1 },
	KeyZ: { octaveDelta: -1 },
	KeyX: { octaveDelta: 1 },
	F9: 'toggleRecord'
};

/** An intent the interface answers itself, because no engine event describes it. */
type LocalIntent =
	| 'zoomIn'
	| 'zoomOut'
	| 'scrollStart'
	| 'openRoll'
	| 'closeRoll'
	| 'splitAtPlayhead'
	| 'splitAtPointer'
	| 'duplicate'
	| 'toggleDetail'
	| 'quantize'
	| 'copy'
	| 'cut'
	| 'paste'
	| 'loopSelection'
	| 'save'
	| 'saveAs'
	| 'open';

/**
 * The minus and equals keys already nudge the volume, so the zoom pair sits on
 * their shifted characters (`+` and `_`) plus the numeric keypad. Shift+Tab
 * switches the detail pane between Clip and Devices, as in Ableton. Enter already
 * adds a block, so the piano roll opens on `P` and closes on Escape, which is
 * free whenever no name is being typed.
 */
const LOCAL: Record<string, LocalIntent> = {
	Home: 'scrollStart',
	KeyP: 'openRoll',
	Escape: 'closeRoll',
	KeyB: 'splitAtPointer',
	NumpadAdd: 'zoomIn',
	NumpadSubtract: 'zoomOut'
};

/**
 * Ctrl (or Cmd on macOS) chords, in the shape Ableton uses for the same edits.
 * Any other chord belongs to the webview and the OS, so it never plays a note
 * or fires a plain command.
 */
const CHORDS: Record<string, LocalIntent> = {
	KeyE: 'splitAtPlayhead',
	KeyD: 'duplicate',
	KeyU: 'quantize',
	KeyC: 'copy',
	KeyX: 'cut',
	KeyV: 'paste',
	KeyL: 'loopSelection',
	KeyO: 'open'
};

/** Chords that carry an engine intent: undo, redo, and add track. */
function chordIntent(event: KeyboardEvent): EngineEvent | null {
	if (event.code === 'KeyZ') return event.shiftKey ? 'redo' : 'undo';
	if (event.code === 'KeyY') return 'redo';
	if (event.code === 'KeyT') return { addTrack: { kind: event.shiftKey ? 'midi' : 'audio' } };
	return null;
}

/** Intents that must fire once per press. Movement, nudging and typing may repeat. */
const ONCE_PER_PRESS = new Set<string>([
	'togglePlay',
	'stop',
	'toggleLoop',
	'toggleRecord',
	'addBlock',
	'deleteBlock',
	'cycleTrackKind',
	'renameBlock',
	'cancelRename',
	'toggleMute',
	'toggleSolo'
]);

const clash = [...Object.keys(COMMANDS), ...Object.keys(LOCAL)].find((code) => code in PIANO);
if (clash) {
	throw new Error(`cassette: ${clash} is bound to both a note and a command`);
}

/** The MIDI pitch a key plays at the given keyboard octave, or null if it is not a note. */
export function pitchFor(code: string, octave: number): number | null {
	const semitone = PIANO[code];
	if (semitone === undefined) return null;
	return Math.min(127, Math.max(0, 60 + octave * 12 + semitone));
}

/**
 * Whether Ctrl or Cmd is held, which turns a key into a chord. AltGr reports
 * itself as Ctrl on Windows, so a character typed with it is not a chord.
 */
export function isChord(event: KeyboardEvent): boolean {
	if (event.getModifierState?.('AltGraph') === true) return false;
	return event.ctrlKey === true || event.metaKey === true;
}

/** The MIDI pitch a keypress plays, or null for a chord or a key that is not a note. */
export function noteFor(event: KeyboardEvent, octave: number): number | null {
	return isChord(event) ? null : pitchFor(event.code, octave);
}

/** Whether holding the key down should fire this intent again. */
export function repeats(intent: EngineEvent): boolean {
	if (typeof intent === 'string') return !ONCE_PER_PRESS.has(intent);
	if (typeof intent === 'object' && intent !== null && 'addTrack' in intent) return false;
	return true;
}

/** The interface-only intent a key carries, or null while nothing is being typed. */
export function localFor(event: KeyboardEvent, renaming: boolean): LocalIntent | null {
	if (renaming) return null;
	if (isChord(event)) {
		if (event.code === 'KeyS') return event.shiftKey ? 'saveAs' : 'save';
		return CHORDS[event.code] ?? null;
	}
	if (event.shiftKey && event.code === 'Equal') return 'zoomIn';
	if (event.shiftKey && event.code === 'Minus') return 'zoomOut';
	if (event.shiftKey && event.code === 'Tab') return 'toggleDetail';
	return LOCAL[event.code] ?? null;
}

/**
 * The engine intent a keypress carries. While the name prompt is open every
 * printable key is text, which is what keeps typing a name from playing notes.
 */
export function intentFor(event: KeyboardEvent, renaming: boolean): EngineEvent | null {
	if (renaming) {
		if (event.key === 'Escape') return 'cancelRename';
		if (event.key === 'Enter') return 'renameBlock';
		if (event.key === 'Backspace') return 'textBackspace';
		if (isChord(event)) return null;
		if (event.key.length === 1) return { textInput: event.key };
		return null;
	}
	if (localFor(event, false) !== null) return null;
	if (isChord(event)) return chordIntent(event);
	return COMMANDS[event.code] ?? null;
}

/** Keys the app owns, so the webview does not scroll or submit on them. */
export function isAppKey(event: KeyboardEvent, renaming: boolean): boolean {
	if (renaming) return true;
	if (localFor(event, false) !== null) return true;
	if (isChord(event)) return chordIntent(event) !== null;
	return event.code in COMMANDS || pitchFor(event.code, 0) !== null;
}
