/**
 * The window keymap. It turns key presses into engine intents and into the
 * intents only the interface answers itself, and it releases every held note when
 * the window loses focus. Nothing here is bound to a component.
 */

import { intentFor, isAppKey, localFor, noteFor, pitchFor, repeats } from './keys';
import type { EngineEvent } from './wire';

/** What the keymap needs from the session it drives. */
interface KeyboardHost {
	readonly renaming: boolean;
	readonly octave: number;
	readonly exporting?: boolean;
	readonly activeView?: 'timeline' | 'mixer';
	selectNextTrack?(): void;
	selectPreviousTrack?(): void;
	dispatch(event: EngineEvent): void;
	sendNow(event: EngineEvent): void;
	setZoom(direction: number): void;
	requestScrollStart(): void;
	openSelectedRoll(): void;
	closeRoll(): void;
	splitSelected(): void;
	splitAtPointer(): void;
	duplicate(): void;
	toggleDetail(): void;
	quantizeSelected(): void;
	saveProject(): void;
	saveProjectAs(): void;
	loadProjectFromDialog(): void;
	copySelection(): void;
	cutSelection(): void;
	pasteSelection(): void;
	delete(): void;
	loopSelection(): void;
	clearSelection(): boolean;
	nudgeSelectedBlock(deltaTrack: number, deltaBeats: number): boolean;
	seekCursor(direction: number): void;
	readonly selectedEffectIndex?: number | null;
	readonly effectClipboard?: unknown | null;
	copyEffect?(track?: number, index?: number): void;
	pasteEffect?(track?: number, index?: number | null): void;
	duplicateEffect?(track?: number, index?: number): void;
	deselectEffect?(): void;
	openTrackRecolor?(): void;
}

/** Controls that keep Shift+Tab for moving focus back, so the keymap leaves it to them. */
const FOCUS_KEEPERS =
	'button, input, select, textarea, a[href], [role="slider"], [role="button"], [role="separator"], [role="tab"]';

/** Elements that own editing and clip-level shortcuts, so arrangement keys must not hijack them. */
function ignoresArrangementKeys(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return false;
	if (target.closest('input, textarea, [contenteditable="true"], [role="slider"]')) return true;
	if (target.closest('[role="grid"]')) return true;
	return false;
}

/** Whether Shift+Tab should move focus instead of switching the detail view. */
function keepsShiftTab(target: EventTarget | null): boolean {
	return target instanceof Element && target.closest(FOCUS_KEEPERS) !== null;
}

/** Velocity the on-screen keyboard plays at. */
const VELOCITY = 100;

/** Wire the keymap to the window. Returns the teardown. */
export function attachKeyboard(host: KeyboardHost): () => void {
	const held = new Set<number>();

	function noteOn(pitch: number): void {
		if (held.has(pitch)) return;
		held.add(pitch);
		host.sendNow({ noteOn: { pitch, velocity: VELOCITY } });
	}

	function noteOff(pitch: number): void {
		if (!held.delete(pitch)) return;
		host.sendNow({ noteOff: { pitch } });
	}

	function releaseAll(): void {
		for (const pitch of [...held]) noteOff(pitch);
	}

	function onKeyDown(event: KeyboardEvent): void {
		if (host.exporting) return;
		const pitch = host.renaming ? null : noteFor(event, host.octave);

		if (pitch !== null) {
			if (!event.repeat) noteOn(pitch);
		} else {
			const inInputOrGrid = ignoresArrangementKeys(event.target);
			const inTextInputOrGrid =
				event.target instanceof Element &&
				(event.target.closest('input:not([type="range"]), textarea, [contenteditable="true"]') !== null ||
					event.target.closest('[role="grid"]') !== null);
			const local = localFor(event, host.renaming);
			if (local === 'toggleDetail' && keepsShiftTab(event.target)) return;
			if (inTextInputOrGrid && (local === 'copy' || local === 'cut' || local === 'paste' || local === 'duplicate' || local === 'loopSelection')) return;

			if (local === 'scrollStart') {
				if (!event.repeat) host.requestScrollStart();
			} else if (local === 'openRoll') {
				if (!event.repeat) host.openSelectedRoll();
			} else if (local === 'closeRoll') {
				if (host.selectedEffectIndex !== null && host.selectedEffectIndex !== undefined && host.deselectEffect) {
					host.deselectEffect();
				} else if (!host.clearSelection()) {
					host.closeRoll();
				}
			} else if (local === 'zoomIn') {
				host.setZoom(1);
			} else if (local === 'zoomOut') {
				host.setZoom(-1);
			} else if (local === 'splitAtPlayhead') {
				if (!event.repeat) host.splitSelected();
			} else if (local === 'splitAtPointer') {
				if (!event.repeat) host.splitAtPointer();
			} else if (local === 'duplicate') {
				if (!event.repeat) {
					if (host.selectedEffectIndex !== null && host.selectedEffectIndex !== undefined && host.duplicateEffect) {
						host.duplicateEffect();
					} else {
						host.duplicate();
					}
				}
			} else if (local === 'toggleDetail') {
				if (!event.repeat) host.toggleDetail();
			} else if (local === 'quantize') {
				if (!event.repeat) host.quantizeSelected();
			} else if (local === 'save') {
				if (!event.repeat) host.saveProject();
			} else if (local === 'saveAs') {
				if (!event.repeat) host.saveProjectAs();
			} else if (local === 'open') {
				if (!event.repeat) host.loadProjectFromDialog();
			} else if (local === 'copy') {
				if (!event.repeat) {
					if (host.selectedEffectIndex !== null && host.selectedEffectIndex !== undefined && host.copyEffect) {
						host.copyEffect();
					} else {
						host.copySelection();
					}
				}
			} else if (local === 'cut') {
				if (!event.repeat) host.cutSelection();
			} else if (local === 'paste') {
				if (!event.repeat) {
					if (host.selectedEffectIndex !== null && host.selectedEffectIndex !== undefined && host.pasteEffect) {
						host.pasteEffect();
					} else if (host.effectClipboard && host.pasteEffect && event.target instanceof Element && event.target.closest('[role="list"][aria-label^="Devices"], [data-effect-card], .scroller')) {
						host.pasteEffect();
					} else {
						host.pasteSelection();
					}
				}
			} else if (local === 'loopSelection') {
				if (!event.repeat) host.loopSelection();
			} else if (local === 'recolorTrack') {
				if (!event.repeat) host.openTrackRecolor?.();
			}

			if (host.activeView === 'mixer' && !inTextInputOrGrid) {
				if (event.key === 'ArrowLeft') {
					host.selectPreviousTrack?.();
					event.preventDefault();
					return;
				}
				if (event.key === 'ArrowRight') {
					host.selectNextTrack?.();
					event.preventDefault();
					return;
				}
			}

			if (!inInputOrGrid && (event.key === 'ArrowUp' || event.key === 'ArrowDown' || event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
				if (event.key === 'ArrowUp') {
					if (!host.nudgeSelectedBlock(-1, 0)) {
						host.dispatch({ moveTrack: -1 });
					}
				} else if (event.key === 'ArrowDown') {
					if (!host.nudgeSelectedBlock(1, 0)) {
						host.dispatch({ moveTrack: 1 });
					}
				} else if (event.key === 'ArrowLeft') {
					if (!host.nudgeSelectedBlock(0, event.altKey ? -0.25 : -1)) {
						host.seekCursor(event.altKey ? -0.25 : -1);
					}
				} else if (event.key === 'ArrowRight') {
					if (!host.nudgeSelectedBlock(0, event.altKey ? 0.25 : 1)) {
						host.seekCursor(event.altKey ? 0.25 : 1);
					}
				}
				event.preventDefault();
				return;
			}

			const intent = intentFor(event, host.renaming);
			if (intent !== null) {
				if (inInputOrGrid && (intent === 'deleteBlock' || event.code === 'Backspace' || event.code === 'Delete')) {
					return;
				}
				if (!event.repeat || repeats(intent)) {
					if (intent === 'deleteBlock') {
						host.delete();
					} else {
						host.dispatch(intent);
					}
				}
			}
		}

		const isSpace = event.code === 'Space' || event.key === ' ';
		const inTextInput =
			event.target instanceof Element &&
			event.target.closest('input:not([type="range"]):not([type="checkbox"]), textarea, [contenteditable="true"]') !== null;

		if (isAppKey(event, host.renaming) && (!ignoresArrangementKeys(event.target) || (isSpace && !inTextInput))) {
			event.preventDefault();
		}
	}

	function onKeyUp(event: KeyboardEvent): void {
		if (!host.renaming) {
			const pitch = pitchFor(event.code, host.octave);
			if (pitch !== null) noteOff(pitch);
		}

		// A focused control runs its Space activation as the default action of
		// keyup, so cancelling only the keydown leaves it free to fire twice.
		const isSpace = event.code === 'Space' || event.key === ' ';
		const inTextInput =
			event.target instanceof Element &&
			event.target.closest('input:not([type="range"]):not([type="checkbox"]), textarea, [contenteditable="true"]') !== null;

		if (isAppKey(event, host.renaming) && (!ignoresArrangementKeys(event.target) || (isSpace && !inTextInput))) {
			event.preventDefault();
		}
	}

	function onBlur(): void {
		releaseAll();
	}

	window.addEventListener('keydown', onKeyDown);
	window.addEventListener('keyup', onKeyUp);
	window.addEventListener('blur', onBlur);

	return () => {
		window.removeEventListener('keydown', onKeyDown);
		window.removeEventListener('keyup', onKeyUp);
		window.removeEventListener('blur', onBlur);
		releaseAll();
	};
}
