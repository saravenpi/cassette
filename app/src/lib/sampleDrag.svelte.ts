/**
 * Dragging a sample out of the file browser and onto a lane. The webview's HTML5
 * drag is unreliable under Tauri, so this drives the gesture from pointer events
 * and exposes the ghost chip's position for the browser to draw.
 */

import { trackDropUnder } from './drag.svelte';
import { gridStep, snapBeats } from './geometry';
import type { SampleEntry } from './wire';

/** What the gesture needs from the browser that owns it. */
interface SampleDropContext {
	zoom(): number;
	drop(entry: SampleEntry, track: number, startBeats: number | null): void;
}

/** Tracks the sample being dragged, and the point the ghost chip follows. */
export class SampleDrag {
	entry = $state<SampleEntry | null>(null);
	ghost = $state<{ x: number; y: number } | null>(null);

	private context: SampleDropContext;
	private startPos = { x: 0, y: 0 };

	constructor(context: SampleDropContext) {
		this.context = context;
	}

	/** Arm a drag from a row. Only a left button starts one. */
	begin(event: PointerEvent, entry: SampleEntry): void {
		if (event.button !== 0) return;
		this.entry = entry;
		this.startPos = { x: event.clientX, y: event.clientY };
		this.ghost = null;
		window.addEventListener('pointermove', this.onMove);
		window.addEventListener('pointerup', this.onEnd);
		window.addEventListener('pointercancel', this.onEnd);
	}

	private onMove = (event: PointerEvent): void => {
		if (this.entry === null) return;
		if (event.buttons === 0) {
			this.stop();
			return;
		}
		if (
			this.ghost === null &&
			Math.abs(event.clientX - this.startPos.x) < 4 &&
			Math.abs(event.clientY - this.startPos.y) < 4
		) {
			return;
		}
		event.preventDefault();
		this.ghost = { x: event.clientX, y: event.clientY };
	};

	private onEnd = (event: PointerEvent): void => {
		const entry = this.entry;
		const dragged = this.ghost !== null;
		const target = dragged
			? trackDropUnder(event.clientX, event.clientY, this.context.zoom())
			: null;
		this.stop();
		if (entry === null || !dragged || target === null || event.type === 'pointercancel') return;
		const startBeats =
			target.beats === null
				? null
				: target.beats < 2.0
					? 0
					: snapBeats(target.beats, gridStep(this.context.zoom()));
		this.context.drop(entry, target.track, startBeats);
	};

	/** Drop the window listeners when the owner goes away mid-drag. */
	destroy(): void {
		this.stop();
	}

	private stop(): void {
		this.entry = null;
		this.ghost = null;
		window.removeEventListener('pointermove', this.onMove);
		window.removeEventListener('pointerup', this.onEnd);
		window.removeEventListener('pointercancel', this.onEnd);
	}
}
