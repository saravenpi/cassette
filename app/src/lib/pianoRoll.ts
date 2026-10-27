/**
 * The geometry of the piano roll: one row per MIDI pitch with 127 at the top,
 * the note grid, hit-testing, the clamps that keep an edited note inside its
 * block, the wheel's velocity step, and the mini preview a block draws. Every
 * function here is pure; beats and pixels convert with `geometry.ts`.
 */

import { beatsToPx, pxToBeats } from './geometry';
import type { BlockView, NoteView } from './wire';

/** The MIDI pitch range, and the pitch the roll scrolls to when it opens. */
export const PITCH_MIN = 0;
export const PITCH_MAX = 127;
export const PITCH_COUNT = PITCH_MAX - PITCH_MIN + 1;
export const MIDDLE_C = 60;

/** Height of one pitch row, and width of the keyboard column, in pixels. */
export const ROW_PX = 12;
export const KEYS_WIDTH = 56;

/** Pixels per beat the roll opens at. */
export const ROLL_ZOOM_DEFAULT = 64;
/** Deepest and widest zoom scales the piano roll allows. */
export const ROLL_ZOOM_MIN = 16;
export const ROLL_ZOOM_MAX = 2048;
export const ROLL_ZOOM_STEP = 1.25;
export const ROLL_WHEEL_ZOOM_RATE = 0.0022;

/** Beats a note snaps to, unless Alt is held: one sixteenth note. */
export const NOTE_GRID = 0.25;
/** Shortest note the engine keeps: one sixty-fourth note. */
export const MIN_NOTE_BEATS = 0.0625;
/** What a click on the empty grid creates. */
export const DEFAULT_NOTE_BEATS = 1;
export const DEFAULT_VELOCITY = 100;
/** Velocity range the engine accepts, and one wheel notch of it. */
export const VELOCITY_MIN = 1;
export const VELOCITY_MAX = 127;
export const VELOCITY_STEP = 8;
/** Width of the grab zone at a note's right edge that resizes it. */
export const HANDLE_PX = 6;
/** Fewest rows the block preview spreads its notes over, so one note is a thin bar. */
export const PREVIEW_MIN_ROWS = 12;

const NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

function clamp(value: number, low: number, high: number): number {
	return Math.min(high, Math.max(low, value));
}

/** Keep a scale inside the piano roll zoom bounds. */
export function rollClampZoom(pxPerBeat: number): number {
	return clamp(pxPerBeat, ROLL_ZOOM_MIN, ROLL_ZOOM_MAX);
}

/** The next piano roll scale one zoom press away, clamped. */
export function rollZoomBy(pxPerBeat: number, direction: number): number {
	return rollClampZoom(direction > 0 ? pxPerBeat * ROLL_ZOOM_STEP : pxPerBeat / ROLL_ZOOM_STEP);
}

/** The piano roll scale after a wheel movement of deltaY pixels, clamped. */
export function rollZoomByWheel(pxPerBeat: number, deltaY: number): number {
	return rollClampZoom(pxPerBeat * Math.exp(-deltaY * ROLL_WHEEL_ZOOM_RATE));
}

/** A pitch held inside the MIDI range and rounded to a whole semitone. */
export function clampPitch(pitch: number): number {
	return clamp(Math.round(pitch), PITCH_MIN, PITCH_MAX);
}

/** The top edge of a pitch's row. */
export function pitchToY(pitch: number, rowPx = ROW_PX): number {
	return (PITCH_MAX - pitch) * rowPx;
}

/** The pitch whose row holds a vertical offset. */
export function yToPitch(y: number, rowPx = ROW_PX): number {
	return clampPitch(PITCH_MAX - Math.floor(y / rowPx));
}

/** A beat moved down to the start of the grid cell it falls in. */
export function floorToGrid(beats: number, grid = NOTE_GRID): number {
	return Math.max(0, Math.floor(beats / grid) * grid);
}

/** A beat rounded to the nearest grid line, or left free when Alt holds the drag. */
export function snapToGrid(beats: number, free = false, grid = NOTE_GRID): number {
	return Math.max(0, free ? beats : Math.round(beats / grid) * grid);
}

/** Whether a pitch is a black key. */
export function isBlackKey(pitch: number): boolean {
	return [1, 3, 6, 8, 10].includes(((pitch % 12) + 12) % 12);
}

/** A pitch's name in scientific notation, where 60 is C4. */
export function noteName(pitch: number): string {
	const semitone = ((pitch % 12) + 12) % 12;
	return `${NAMES[semitone]}${Math.floor(pitch / 12) - 1}`;
}

/** Which edge of a note is under a point, or null for its body. */
export type NoteEdge = 'start' | 'end' | null;

/** Which note sits under a point, and whether the point is on an edge handle. */
export interface NoteHit {
	note: number;
	edge: NoteEdge;
}

/**
 * The note under a point of the grid, searching from the last note drawn so the
 * one on top wins. The start handle drags to resize from the left, the end handle
 * from the right, and the body in between moves. Handles shrink to half the width
 * on narrow notes so they never overlap.
 */
export function hitNote(
	notes: readonly NoteView[],
	x: number,
	y: number,
	pxPerBeat: number,
	rowPx = ROW_PX
): NoteHit | null {
	if (y < 0 || y >= PITCH_COUNT * rowPx) return null;
	const pitch = yToPitch(y, rowPx);
	for (let index = notes.length - 1; index >= 0; index -= 1) {
		const note = notes[index];
		if (note.pitch !== pitch) continue;
		const left = beatsToPx(note.startBeats, pxPerBeat);
		const right = left + beatsToPx(note.lengthBeats, pxPerBeat);
		if (x < left || x > right) continue;
		const width = right - left;
		const handle = Math.min(HANDLE_PX, width / 2);
		let edge: NoteEdge = null;
		if (x - left < handle) {
			edge = 'start';
		} else if (right - x <= handle) {
			edge = 'end';
		}
		return { note: index, edge };
	}
	return null;
}

/** Where a moved note may land: a real pitch, and a start that keeps it inside the block. */
export function clampMove(
	pitch: number,
	startBeats: number,
	lengthBeats: number,
	blockBeats: number
): { pitch: number; startBeats: number } {
	const latest = Math.max(0, blockBeats - Math.min(lengthBeats, blockBeats));
	return { pitch: clampPitch(pitch), startBeats: clamp(startBeats, 0, latest) };
}

/** How long a resized note may be: at least the engine's minimum, at most the rest of the block. */
export function clampLength(startBeats: number, lengthBeats: number, blockBeats: number): number {
	return clamp(lengthBeats, MIN_NOTE_BEATS, Math.max(MIN_NOTE_BEATS, blockBeats - startBeats));
}

/** A velocity one wheel notch away: up when the wheel rolls away, one step at a time with Shift. */
export function velocityStep(velocity: number, deltaY: number, fine = false): number {
	const step = (fine ? 1 : VELOCITY_STEP) * -Math.sign(deltaY);
	return clamp(Math.round(velocity + step), VELOCITY_MIN, VELOCITY_MAX);
}

/** How opaque a note is drawn, so a quiet note reads lighter than a loud one. */
export function velocityOpacity(velocity: number): number {
	return 0.3 + 0.7 * (clamp(velocity, VELOCITY_MIN, VELOCITY_MAX) / VELOCITY_MAX);
}

/** The beat under a horizontal offset of the grid, with the pitch under a vertical one. */
export function cellAt(
	x: number,
	y: number,
	pxPerBeat: number,
	rowPx = ROW_PX
): { beats: number; pitch: number } {
	return { beats: Math.max(0, pxToBeats(x, pxPerBeat)), pitch: yToPitch(y, rowPx) };
}

/**
 * The rows a block preview spreads its notes over: `top` is the pitch drawn in
 * row zero. A narrow range is widened around its middle to `PREVIEW_MIN_ROWS`.
 */
export function previewRange(notes: readonly NoteView[]): { top: number; rows: number } {
	if (notes.length === 0) return { top: MIDDLE_C + PREVIEW_MIN_ROWS / 2, rows: PREVIEW_MIN_ROWS };
	const pitches = notes.map((note) => note.pitch);
	const low = Math.min(...pitches);
	const high = Math.max(...pitches);
	const rows = Math.max(PREVIEW_MIN_ROWS, high - low + 1);
	const spare = rows - (high - low + 1);
	return { top: high + Math.ceil(spare / 2), rows };
}

/**
 * Where the open block is in this snapshot: the same index while it is still a
 * MIDI block with the same name or start, otherwise the nearest MIDI block of
 * that name on the track, which is where a move re-sorts it to. Null when gone.
 */
export function locateBlock(
	blocks: readonly BlockView[],
	hint: number,
	name: string,
	startBeats: number
): number | null {
	const here = blocks[hint];
	const kept = here?.kind === 'midi' && (here.name === name || here.startBeats === startBeats);
	if (kept) return hint;
	let best: number | null = null;
	let distance = Infinity;
	blocks.forEach((block, index) => {
		if (block.kind !== 'midi' || block.name !== name) return;
		const away = Math.abs(block.startBeats - startBeats);
		if (away < distance) {
			distance = away;
			best = index;
		}
	});
	return best;
}
