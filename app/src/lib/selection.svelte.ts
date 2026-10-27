/**
 * Reactive time selection state for the arrangement view.
 */

import { extendRange, normalizeRange, resizeRangeEnd, resizeRangeStart } from './selection';
import type { BeatRange } from './wire';

interface DragAnchor {
	track: number;
	beats: number;
}

class SelectionStore {
	range = $state<BeatRange | null>(null);
	dragging = $state(false);
	resizing = $state<'start' | 'end' | null>(null);

	private anchor: DragAnchor | null = null;
	private moved = false;

	/** Set the active selection range directly. */
	setRange(range: BeatRange | null): void {
		this.range = range;
	}

	/** Clear the active selection. */
	clear(): void {
		this.range = null;
		this.anchor = null;
		this.dragging = false;
		this.resizing = null;
		this.moved = false;
	}

	/** Select the span of one block on its track. */
	selectBlockSpan(track: number, startBeats: number, lengthBeats: number): void {
		this.range = {
			firstTrack: track,
			lastTrack: track,
			startBeats,
			endBeats: startBeats + lengthBeats
		};
	}

	/** Extend the existing selection to encompass a clicked track and beat. */
	extend(track: number, beats: number): void {
		this.range = extendRange(this.range, track, beats);
	}

	/** Start drawing a time selection rectangle from an anchor point. */
	beginDrag(track: number, beats: number): void {
		this.anchor = { track, beats };
		this.dragging = true;
		this.moved = false;
	}

	/** Update the active drag rectangle. */
	updateDrag(track: number, beats: number): void {
		if (this.anchor === null) return;
		this.moved = true;
		this.range = normalizeRange(this.anchor.track, track, this.anchor.beats, beats);
	}

	/** Complete a drag gesture; returns true if a rectangle was drawn, false for a plain click. */
	finishDrag(): boolean {
		const hadMovement = this.moved && this.range !== null && this.range.startBeats < this.range.endBeats;
		this.anchor = null;
		this.dragging = false;
		this.moved = false;
		if (!hadMovement) {
			this.range = null;
		}
		return hadMovement;
	}

	/** Start dragging one of the selection edge resize handles. */
	beginResize(edge: 'start' | 'end'): void {
		if (this.range === null) return;
		this.resizing = edge;
	}

	/** Update the edge being resized to the given beat. */
	updateResize(beats: number): void {
		if (this.range === null || this.resizing === null) return;
		if (this.resizing === 'start') {
			this.range = resizeRangeStart(this.range, beats);
		} else {
			this.range = resizeRangeEnd(this.range, beats);
		}
	}

	/** Finish resizing a selection edge. */
	finishResize(): void {
		this.resizing = null;
	}
}

export const selection = new SelectionStore();
