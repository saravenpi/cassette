/**
 * The drags the timeline answers: moving a block along its lane or across
 * lanes, dragging its right edge to resize or, with Shift on an audio block, to
 * stretch it, and dragging its left edge to trim its start. The controller
 * owns the optimistic geometry the lane draws, sends at most one message per
 * frame, and follows the block when the engine re-sorts the lane under it.
 */

import {
	MIN_LENGTH_BEATS,
	gridStep,
	pxToBeats,
	snapBeats,
	stretchLength,
	trimSpan
} from './geometry';
import type { BlockView, EngineEvent, UiState } from './wire';

/** Which part of a block a drag moves. */
export type DragKind = 'move' | 'resize' | 'trim' | 'stretch';

/** A block being dragged, with the geometry drawn until the engine confirms. */
export interface Drag {
	kind: DragKind;
	view: BlockView;
	name: string;
	track: number;
	index: number;
	targetTrack: number;
	startBeats: number;
	lengthBeats: number;
	grab: number;
	floorBeats: number;
	released: boolean;
	dirty: boolean;
	releasedAt: number;
	invalidTarget: boolean;
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
	const elements = typeof document !== 'undefined' && document.elementsFromPoint ? document.elementsFromPoint(clientX, clientY) : [];
	for (const hit of elements) {
		const lane =
			typeof Element !== 'undefined' && hit instanceof Element
				? hit.closest('[data-lane-track]')
				: (hit as any)?.closest?.('[data-lane-track]');
		if (lane && typeof lane === 'object' && 'dataset' in lane) {
			const track = Number((lane as any).dataset?.laneTrack);
			if (Number.isInteger(track) && typeof (lane as any).getBoundingClientRect === 'function') {
				return { track, beats: pxToBeats(clientX - (lane as any).getBoundingClientRect().left, zoom) };
			}
		}
	}
	return null;
}

/**
 * The track drop target under a viewport point: a timeline lane (with its beat),
 * a track header, or a mixer channel strip.
 */
export function trackDropUnder(
	clientX: number,
	clientY: number,
	zoom: number
): { track: number; beats: number | null } | null {
	const lane = laneUnder(clientX, clientY, zoom);
	if (lane !== null) return lane;
	const elements =
		typeof document !== 'undefined' && document.elementsFromPoint
			? document.elementsFromPoint(clientX, clientY)
			: [];
	for (const hit of elements) {
		const header =
			typeof Element !== 'undefined' && hit instanceof Element
				? hit.closest('[data-track-header], [data-strip-index]')
				: (hit as any)?.closest?.('[data-track-header], [data-strip-index]');
		if (header && typeof header === 'object' && 'dataset' in header) {
			const ds = (header as any).dataset;
			const raw = ds?.trackHeader ?? ds?.stripIndex;
			const track = Number(raw);
			if (raw !== undefined && Number.isInteger(track) && track >= 0) {
				return { track, beats: null };
			}
		}
	}
	return null;
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

	/** Start dragging a block's right edge; `stretch` rescales an audio block's material. */
	beginResize(track: number, index: number, view: BlockView, stretch = false): void {
		this.begin(stretch && view.kind === 'audio' ? 'stretch' : 'resize', track, index, view, 0);
	}

	/** Start dragging a block's left edge, keeping its end where it is. */
	beginTrim(track: number, index: number, view: BlockView): void {
		this.begin('trim', track, index, view, 0);
	}

	/** Abort any active drag immediately without sending pending moves. */
	cancel(): void {
		this.state = null;
		window.removeEventListener('pointermove', this.onPointerMove);
		window.removeEventListener('pointerup', this.onPointerUp);
		window.removeEventListener('pointercancel', this.onPointerUp);
	}

	/**
	 * Send the pending drag position, at most once per frame. A MIDI trim waits
	 * for the release: the engine drops the notes a trim uncovers, so trimming
	 * past a note and back would otherwise lose it.
	 */
	async flush(): Promise<void> {
		const drag = this.state;
		if (drag === null || !drag.dirty) return;
		if (drag.kind === 'trim' && drag.view.kind === 'midi' && !drag.released) return;
		if (drag.invalidTarget) {
			drag.targetTrack = drag.track;
		}
		drag.dirty = false;
		await this.context.send(request(drag));
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
			Math.abs(current.startBeats - drag.startBeats) < SETTLE_EPSILON &&
			Math.abs(current.lengthBeats - drag.lengthBeats) < SETTLE_EPSILON;
		if (confirmed || performance.now() - drag.releasedAt > SETTLE_TIMEOUT_MS) this.state = null;
	}

	private begin(kind: DragKind, track: number, index: number, view: BlockView, grab: number): void {
		void this.context.send('checkpoint');
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
			floorBeats: this.floor(view),
			released: false,
			dirty: false,
			releasedAt: 0,
			invalidTarget: false
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
			const startBeats = snapBeats(lane.beats - drag.grab, gridStep(this.context.zoom()), event.altKey);
			const targetTrackKind = this.context.snapshot()?.tracks[lane.track]?.kind;
			const wasInvalid = drag.invalidTarget;
			if (drag.view.kind !== targetTrackKind) {
				drag.invalidTarget = true;
			} else {
				drag.invalidTarget = false;
				drag.targetTrack = lane.track;
			}
			if (startBeats === drag.startBeats && (drag.invalidTarget ? wasInvalid : lane.track === drag.targetTrack)) return;
			drag.startBeats = startBeats;
		} else if (drag.kind === 'trim') {
			const span = trimSpan(
				snapBeats(lane.beats, gridStep(this.context.zoom()), event.altKey),
				drag.startBeats + drag.lengthBeats,
				drag.floorBeats
			);
			if (span.startBeats === drag.startBeats) return;
			drag.startBeats = span.startBeats;
			drag.lengthBeats = span.lengthBeats;
		} else {
			const end = snapBeats(lane.beats, gridStep(this.context.zoom()), event.altKey);
			const wanted = Math.max(MIN_LENGTH_BEATS, end - drag.startBeats);
			const lengthBeats =
				drag.kind === 'stretch' ? stretchLength(drag.view.lengthBeats, drag.view.rate, wanted) : wanted;
			if (lengthBeats === drag.lengthBeats) return;
			drag.lengthBeats = lengthBeats;
		}
		drag.dirty = true;
	};

	/**
	 * The earliest beat a trim may pull a block's start back to: the top of the
	 * song, or for an audio block the beat where its file begins.
	 */
	private floor(view: BlockView): number {
		const bpm = this.context.snapshot()?.bpm ?? 0;
		if (view.kind !== 'audio' || bpm <= 0 || view.rate <= 0) return 0;
		return Math.max(0, view.startBeats - (view.offsetSeconds * bpm) / 60 / view.rate);
	}

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
		if (drag.invalidTarget) {
			drag.targetTrack = drag.track;
		}
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

/** The engine message that carries a drag's current geometry. */
function request(drag: Drag): EngineEvent {
	const at = { track: drag.track, block: drag.index };
	switch (drag.kind) {
		case 'move':
			return { placeBlock: { ...at, targetTrack: drag.targetTrack, startBeats: drag.startBeats } };
		case 'trim':
			return { trimBlockStart: { ...at, startBeats: drag.startBeats } };
		case 'stretch':
			return { stretchBlock: { ...at, lengthBeats: drag.lengthBeats } };
		default:
			return { resizeBlock: { ...at, lengthBeats: drag.lengthBeats } };
	}
}
