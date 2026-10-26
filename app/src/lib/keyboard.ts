/**
 * The window keymap. It turns key presses into engine intents and into the
 * intents only the interface answers itself, and it releases every held note when
 * the window loses focus. Nothing here is bound to a component.
 */

import { intentFor, isAppKey, localFor, pitchFor, repeats } from './keys';
import type { EngineEvent } from './wire';

/** What the keymap needs from the session it drives. */
interface KeyboardHost {
	readonly renaming: boolean;
	readonly octave: number;
	dispatch(event: EngineEvent): void;
	sendNow(event: EngineEvent): void;
	setZoom(direction: number): void;
	requestScrollStart(): void;
	openSelectedRoll(): void;
	closeRoll(): void;
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
		const pitch = host.renaming ? null : pitchFor(event.code, host.octave);

		if (pitch !== null) {
			if (!event.repeat) noteOn(pitch);
		} else {
			const local = localFor(event, host.renaming);
			if (local === 'scrollStart') {
				if (!event.repeat) host.requestScrollStart();
			} else if (local === 'openRoll') {
				if (!event.repeat) host.openSelectedRoll();
			} else if (local === 'closeRoll') {
				host.closeRoll();
			} else if (local === 'zoomIn') {
				host.setZoom(1);
			} else if (local === 'zoomOut') {
				host.setZoom(-1);
			}

			const intent = intentFor(event, host.renaming);
			if (intent !== null && (!event.repeat || repeats(intent))) host.dispatch(intent);
		}

		if (isAppKey(event, host.renaming)) event.preventDefault();
	}

	function onKeyUp(event: KeyboardEvent): void {
		if (!host.renaming) {
			const pitch = pitchFor(event.code, host.octave);
			if (pitch !== null) noteOff(pitch);
		}

		// A focused control runs its Space activation as the default action of
		// keyup, so cancelling only the keydown leaves it free to fire twice.
		if (isAppKey(event, host.renaming)) event.preventDefault();
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
