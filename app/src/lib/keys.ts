/**
 * The webview keymap: the piano row, the engine commands, and the two zoom keys
 * and scroll key that only the interface itself handles.
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
	KeyX: { octaveDelta: 1 }
};

/** An intent the interface answers itself, because no engine event describes it. */
type LocalIntent = 'zoomIn' | 'zoomOut' | 'scrollStart';

/**
 * The minus and equals keys already nudge the volume, so the zoom pair sits on
 * their shifted characters (`+` and `_`) plus the numeric keypad.
 */
const LOCAL: Record<string, LocalIntent> = {
	Home: 'scrollStart',
	NumpadAdd: 'zoomIn',
	NumpadSubtract: 'zoomOut'
};

/** Intents that must fire once per press. Movement, nudging and typing may repeat. */
const ONCE_PER_PRESS = new Set<string>([
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
]);

const clash = Object.keys(COMMANDS).find((code) => code in PIANO);
if (clash) {
	throw new Error(`zboub69: ${clash} is bound to both a note and a command`);
}

/** The MIDI pitch a key plays at the given keyboard octave, or null if it is not a note. */
export function pitchFor(code: string, octave: number): number | null {
	const semitone = PIANO[code];
	if (semitone === undefined) return null;
	return Math.min(127, Math.max(0, 60 + octave * 12 + semitone));
}

/** Whether holding the key down should fire this intent again. */
export function repeats(intent: EngineEvent): boolean {
	return typeof intent === 'string' ? !ONCE_PER_PRESS.has(intent) : true;
}

/** The interface-only intent a key carries, or null while nothing is being typed. */
export function localFor(event: KeyboardEvent, renaming: boolean): LocalIntent | null {
	if (renaming) return null;
	if (event.shiftKey && event.code === 'Equal') return 'zoomIn';
	if (event.shiftKey && event.code === 'Minus') return 'zoomOut';
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
		if (event.key.length === 1) return { textInput: event.key };
		return null;
	}
	if (localFor(event, false) !== null) return null;
	return COMMANDS[event.code] ?? null;
}

/** Keys the app owns, so the webview does not scroll or submit on them. */
export function isAppKey(event: KeyboardEvent, renaming: boolean): boolean {
	if (renaming) return true;
	if (localFor(event, false) !== null) return true;
	return event.code in COMMANDS || pitchFor(event.code, 0) !== null;
}
