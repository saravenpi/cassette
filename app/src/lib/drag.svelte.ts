/**
 * The two drags the timeline answers: moving a block along its lane or across
 * lanes, and dragging its right edge. The controller owns the optimistic geometry
 * the lane draws, sends at most one message per frame, and follows the block when
 * the engine re-sorts the lane under it.
 */

import { MIN_LENGTH_BEATS, pxToBeats, snapBeats } from './geometry';
import type { BlockView, EngineEvent, UiState } from './wire';

/** A block being dragged, with the geometry drawn until the engine confirms. */
export interface Drag {
	kind: 'move' | 'resize';
	view: BlockView;
	name: string;
	track: number;
	index: number;
	targetTrack: number;
	startBeats: number;
	lengthBeats: number;
	grab: number;
	released: boolean;
	dirty: boolean;
	releasedAt: number;
}

/** What a drag needs from the session it belongs to. */
interface DragContext {
	snapshot(): UiState | null;
	zoom(): number;
	send(event: EngineEvent): Promise<boolean>;
}

/** How long a released drag may keep overriding the snapshot before it gives up. */
const SETTLE_TIMEOUT_MS = 250;
/** How close the engine has to land to count as having confirmed a drag. */
const SETTLE_EPSILON = 0.01;

/** The lane under a viewport point, with the raw beat under it. */
export function laneUnder(
	clientX: number,
	clientY: number,
	zoom: number
): { track: number; beats: number } | null {
	const hit = document.elementFromPoint(clientX, clientY);
	const lane = hit instanceof Element ? hit.closest('[data-lane-track]') : null;
	if (!(lane instanceof HTMLElement)) return null;
	const track = Number(lane.dataset.laneTrack);
	if (!Number.isInteger(track)) return null;
	return { track, beats: pxToBeats(clientX - lane.getBoundingClientRect().left, zoom) };
}

/** Tracks the block drags, and holds the one that is running. */
export class DragController {
	state = $state<Drag | null>(null);

	private context: DragContext;

	constructor(context: DragContext) {
		this.context = context;
	}

	/** Start dragging a block along its lane, `grab` beats from its left edge. */
	beginMove(track: number, index: number, view: BlockView, grab: number): void {
		this.begin('move', track, index, view, grab);
	}

	/** Start dragging a block's right edge. */
	beginResize(track: number, index: number, view: BlockView): void {
		this.begin('resize', track, index, view, 0);
	}

	/** Send the pending drag position, at most once per frame. */
	async flush(): Promise<void> {
		const drag = this.state;
		if (drag === null || !drag.dirty) return;
		drag.dirty = false;
		const request: EngineEvent =
			drag.kind === 'move'
				? {
						placeBlock: {
							track: drag.track,
							block: drag.index,
							targetTrack: drag.targetTrack,
							startBeats: drag.startBeats
						}
					}
				: { resizeBlock: { track: drag.track, block: drag.index, lengthBeats: drag.lengthBeats } };
		await this.context.send(request);
	}

	/** Follow the block across frames, and drop the drag once the engine agrees. */
	settle(): void {
		const drag = this.state;
		if (drag === null) return;
		this.locate(drag);
		if (!drag.released || drag.dirty) return;
		const current = this.context.snapshot()?.tracks[drag.targetTrack]?.blocks[drag.index];
		const confirmed =
			current !== undefined &&
			current.name === drag.name &&
			(drag.kind === 'move'
				? Math.abs(current.startBeats - drag.startBeats) < SETTLE_EPSILON
				: Math.abs(current.lengthBeats - drag.lengthBeats) < SETTLE_EPSILON);
		if (confirmed || performance.now() - drag.releasedAt > SETTLE_TIMEOUT_MS) this.state = null;
	}

	private begin(kind: 'move' | 'resize', track: number, index: number, view: BlockView, grab: number): void {
		this.state = {
			kind,
			view: { ...view, selected: true },
			name: view.name,
			track,
			index,
			targetTrack: track,
			startBeats: view.startBeats,
			lengthBeats: view.lengthBeats,
			grab,
			released: false,
			dirty: false,
			releasedAt: 0
		};
		window.addEventListener('pointermove', this.onPointerMove);
		window.addEventListener('pointerup', this.onPointerUp);
		window.addEventListener('pointercancel', this.onPointerUp);
	}

	private onPointerMove = (event: PointerEvent): void => {
		const drag = this.state;
		if (drag === null) return;
		if (event.buttons === 0) {
			this.release();
			return;
		}
		const lane = laneUnder(event.clientX, event.clientY, this.context.zoom());
		if (lane === null) return;
		if (drag.kind === 'move') {
			const startBeats = snapBeats(lane.beats - drag.grab, event.altKey);
			if (startBeats === drag.startBeats && lane.track === drag.targetTrack) return;
			drag.startBeats = startBeats;
			drag.targetTrack = lane.track;
		} else {
			const end = snapBeats(lane.beats, event.altKey);
			const lengthBeats = Math.max(MIN_LENGTH_BEATS, end - drag.startBeats);
			if (lengthBeats === drag.lengthBeats) return;
			drag.lengthBeats = lengthBeats;
		}
		drag.dirty = true;
	};

	private onPointerUp = (): void => {
		this.release();
	};

	/**
	 * The button came up, or a move arrived with no button held, which is what a
	 * release outside the window looks like. Either way the drag stops moving and
	 * only the engine's confirmation is left to wait for.
	 */
	private release(): void {
		const drag = this.state;
		if (drag === null) return;
		drag.released = true;
		drag.releasedAt = performance.now();
		window.removeEventListener('pointermove', this.onPointerMove);
		window.removeEventListener('pointerup', this.onPointerUp);
		window.removeEventListener('pointercancel', this.onPointerUp);
	}

	/**
	 * Re-point the drag at wherever the engine has put the block: a move re-sorts
	 * the lane, so the next message would otherwise name the wrong block. The name
	 * plus the nearest start is what identifies it, since the engine keeps a
	 * block's name when it moves it.
	 */
	private locate(drag: Drag): void {
		const snapshot = this.context.snapshot();
		if (snapshot === null) return;
		const found =
			this.findBlock(snapshot, drag.targetTrack, drag) ?? this.findBlock(snapshot, drag.track, drag);
		if (found === null) return;
		drag.track = found.track;
		drag.index = found.index;
	}

	private findBlock(
		snapshot: UiState,
		lane: number,
		drag: Drag
	): { track: number; index: number } | null {
		const track = snapshot.tracks[lane];
		if (track === undefined) return null;
		let best: number | null = null;
		let distance = Infinity;
		track.blocks.forEach((block, index) => {
			if (block.name !== drag.name) return;
			const away = Math.abs(block.startBeats - drag.startBeats);
			if (away < distance) {
				distance = away;
				best = index;
			}
		});
		return best === null ? null : { track: lane, index: best };
	}
}
