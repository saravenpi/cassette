/**
 * Dragging an effect out of the effect browser and into a device chain. Tauri's
 * file-drop handler can swallow HTML5 drags, so the gesture runs on pointer
 * events: it arms on pointerdown, starts once the pointer travels a few pixels
 * (so a double-click never flashes a ghost), and on release hands the point to
 * whichever chain registered itself as the drop target.
 */

import type { EffectKind } from './wire';

/** Pixels the pointer travels before a press turns into a drag. */
const THRESHOLD_PX = 4;

/** Where an effect drops: the chain decides whether the point lies over it. */
export type EffectDropTarget = (kind: EffectKind, x: number, y: number) => void;

/** The effect being dragged, the point its ghost follows, and the chain it may drop on. */
class EffectDrag {
	kind = $state<EffectKind | null>(null);
	name = $state('');
	ghost = $state<{ x: number; y: number } | null>(null);
	target: EffectDropTarget | null = null;
	onStart: (() => void) | null = null;

	private origin = { x: 0, y: 0 };
	private armed = false;

	/** Arm a drag from a browser row. Only a left button arms one. */
	begin(event: PointerEvent, kind: EffectKind, name: string): void {
		if (event.button !== 0) return;
		this.stop();
		this.armed = true;
		this.kind = kind;
		this.name = name;
		this.origin = { x: event.clientX, y: event.clientY };
		window.addEventListener('pointermove', this.onMove);
		window.addEventListener('pointerup', this.onEnd);
		window.addEventListener('pointercancel', this.onEnd);
	}

	private onMove = (event: PointerEvent): void => {
		if (!this.armed) return;
		if (event.buttons === 0) {
			this.stop();
			return;
		}
		if (this.ghost === null) {
			const moved = Math.hypot(event.clientX - this.origin.x, event.clientY - this.origin.y);
			if (moved < THRESHOLD_PX) return;
			this.onStart?.();
		}
		this.ghost = { x: event.clientX, y: event.clientY };
	};

	private onEnd = (event: PointerEvent): void => {
		const kind = this.kind;
		const dragging = this.ghost !== null;
		this.stop();
		if (kind === null || !dragging || event.type === 'pointercancel') return;
		this.target?.(kind, event.clientX, event.clientY);
	};

	/** Drop the window listeners, ending any drag in flight. */
	stop(): void {
		this.armed = false;
		this.kind = null;
		this.ghost = null;
		window.removeEventListener('pointermove', this.onMove);
		window.removeEventListener('pointerup', this.onEnd);
		window.removeEventListener('pointercancel', this.onEnd);
	}
}

export const effectDrag = new EffectDrag();
