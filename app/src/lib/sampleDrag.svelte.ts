/**
 * Dragging a sample out of the file browser and onto a lane. The webview's HTML5
 * drag is unreliable under Tauri, so this drives the gesture from pointer events
 * and exposes the ghost chip's position for the browser to draw.
 */

import { laneUnder } from './drag.svelte';
import { snapBeats } from './geometry';
import type { SampleEntry } from './wire';

/** What the gesture needs from the browser that owns it. */
interface SampleDropContext {
	zoom(): number;
	drop(entry: SampleEntry, track: number, startBeats: number): void;
}

/** Tracks the sample being dragged, and the point the ghost chip follows. */
export class SampleDrag {
	entry = $state<SampleEntry | null>(null);
	ghost = $state<{ x: number; y: number } | null>(null);

	private context: SampleDropContext;

	constructor(context: SampleDropContext) {
		this.context = context;
	}

	/** Arm a drag from a row. Only a left button starts one. */
	begin(event: PointerEvent, entry: SampleEntry): void {
		if (event.button !== 0) return;
		this.entry = entry;
		this.ghost = { x: event.clientX, y: event.clientY };
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
		this.ghost = { x: event.clientX, y: event.clientY };
	};

	private onEnd = (event: PointerEvent): void => {
		const entry = this.entry;
		const lane = laneUnder(event.clientX, event.clientY, this.context.zoom());
		this.stop();
		if (entry === null || lane === null || event.type === 'pointercancel') return;
		const startBeats = lane.beats < 2.0 ? 0 : snapBeats(lane.beats);
		this.context.drop(entry, lane.track, startBeats);
	};

	private stop(): void {
		this.entry = null;
		this.ghost = null;
		window.removeEventListener('pointermove', this.onMove);
		window.removeEventListener('pointerup', this.onEnd);
		window.removeEventListener('pointercancel', this.onEnd);
	}
}
