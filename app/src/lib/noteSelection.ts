/**
 * Selection, group edits and clipboard operations for notes in the piano roll.
 * All math here is pure and preserves chord shape and relative timing.
 */

import { clampLength, MIN_NOTE_BEATS, PITCH_MAX, PITCH_MIN } from './pianoRoll';
import type { NoteEdit, NoteView } from './wire';

/** Beat and pitch rectangle for marquee selection. */
export interface MarqueeBox {
	startBeat: number;
	endBeat: number;
	startPitch: number;
	endPitch: number;
}

/** One note stored in the clipboard, relative to the earliest start. */
export interface ClipboardNote {
	pitch: number;
	startOffset: number;
	lengthBeats: number;
	velocity: number;
}

/** The piano roll clipboard snapshot. */
export interface NoteClipboard {
	notes: ClipboardNote[];
	spanBeats: number;
	latestEnd: number;
}

let moduleClipboard: NoteClipboard | null = null;

/** Read the module-level note clipboard. */
export function getModuleClipboard(): NoteClipboard | null {
	return moduleClipboard;
}

/** Write the module-level note clipboard. */
export function setModuleClipboard(clipboard: NoteClipboard | null): void {
	moduleClipboard = clipboard;
}

/** Find all note indices intersecting a beat and pitch box. */
export function marqueeSelect(notes: readonly NoteView[], box: MarqueeBox): Set<number> {
	const minBeat = Math.min(box.startBeat, box.endBeat);
	const maxBeat = Math.max(box.startBeat, box.endBeat);
	const minPitch = Math.min(box.startPitch, box.endPitch);
	const maxPitch = Math.max(box.startPitch, box.endPitch);
	const hits = new Set<number>();
	for (let i = 0; i < notes.length; i += 1) {
		const note = notes[i];
		if (note.pitch < minPitch || note.pitch > maxPitch) continue;
		const noteEnd = note.startBeats + note.lengthBeats;
		const overlaps =
			maxBeat > minBeat
				? note.startBeats < maxBeat && noteEnd > minBeat
				: note.startBeats <= minBeat && noteEnd >= minBeat;
		if (overlaps) hits.add(i);
	}
	return hits;
}

/** Toggle one note index in or out of a selection set. */
export function toggleNote(selection: ReadonlySet<number>, index: number): Set<number> {
	const next = new Set(selection);
	if (next.has(index)) next.delete(index);
	else next.add(index);
	return next;
}

/** Union a selection set with an incoming sequence of indices. */
export function addSelection(
	selection: ReadonlySet<number>,
	incoming: Iterable<number>
): Set<number> {
	const next = new Set(selection);
	for (const index of incoming) next.add(index);
	return next;
}

/** Create a set with a contiguous range of indices. */
export function rangeSelection(start: number, count: number): Set<number> {
	const result = new Set<number>();
	for (let i = 0; i < count; i += 1) result.add(start + i);
	return result;
}

/** Remap a selection set after a batch deletion of notes. */
export function remapAfterDelete(
	selection: ReadonlySet<number>,
	deletedIndices: readonly number[]
): Set<number> {
	const deletedSet = new Set(deletedIndices);
	const sortedDeleted = Array.from(deletedSet).sort((a, b) => a - b);
	const result = new Set<number>();
	for (const index of selection) {
		if (deletedSet.has(index)) continue;
		let shift = 0;
		for (const d of sortedDeleted) {
			if (d < index) shift += 1;
			else break;
		}
		result.add(index - shift);
	}
	return result;
}

/** Clamp group move deltas so no note leaves the block or pitch bounds. */
export function clampGroupMove(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaPitch: number,
	deltaBeats: number,
	blockBeats: number
): { deltaPitch: number; deltaBeats: number } {
	const indices = Array.from(selection).filter((i) => notes[i] !== undefined);
	if (indices.length === 0) return { deltaPitch: 0, deltaBeats: 0 };
	let minPitch = PITCH_MAX;
	let maxPitch = PITCH_MIN;
	let minStart = Infinity;
	let maxEnd = -Infinity;
	for (const i of indices) {
		const note = notes[i];
		if (note.pitch < minPitch) minPitch = note.pitch;
		if (note.pitch > maxPitch) maxPitch = note.pitch;
		if (note.startBeats < minStart) minStart = note.startBeats;
		const end = note.startBeats + note.lengthBeats;
		if (end > maxEnd) maxEnd = end;
	}
	const minDeltaPitch = PITCH_MIN - minPitch;
	const maxDeltaPitch = PITCH_MAX - maxPitch;
	const clampedDeltaPitch = Math.min(
		maxDeltaPitch,
		Math.max(minDeltaPitch, Math.round(deltaPitch))
	);
	const minDeltaBeats = -minStart;
	const maxDeltaBeats = Math.max(minDeltaBeats, blockBeats - maxEnd);
	const clampedDeltaBeats = Math.min(maxDeltaBeats, Math.max(minDeltaBeats, deltaBeats));
	return { deltaPitch: clampedDeltaPitch, deltaBeats: clampedDeltaBeats };
}

/** Clamp left-edge delta keeping each note's end fixed and above minimum length. */
export function clampGroupResizeLeft(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaBeats: number
): number {
	const indices = Array.from(selection).filter((i) => notes[i] !== undefined);
	if (indices.length === 0) return 0;
	let minStart = Infinity;
	let minLength = Infinity;
	for (const i of indices) {
		const note = notes[i];
		if (note.startBeats < minStart) minStart = note.startBeats;
		if (note.lengthBeats < minLength) minLength = note.lengthBeats;
	}
	const minDelta = -minStart;
	const maxDelta = Math.max(minDelta, minLength - MIN_NOTE_BEATS);
	return Math.min(maxDelta, Math.max(minDelta, deltaBeats));
}

/** Clamp right-edge delta keeping starts fixed, above minimum length, inside block. */
export function clampGroupResizeRight(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaBeats: number,
	blockBeats: number
): number {
	const indices = Array.from(selection).filter((i) => notes[i] !== undefined);
	if (indices.length === 0) return 0;
	let minLength = Infinity;
	let maxEnd = -Infinity;
	for (const i of indices) {
		const note = notes[i];
		if (note.lengthBeats < minLength) minLength = note.lengthBeats;
		const end = note.startBeats + note.lengthBeats;
		if (end > maxEnd) maxEnd = end;
	}
	const minDelta = MIN_NOTE_BEATS - minLength;
	const maxDelta = Math.max(minDelta, blockBeats - maxEnd);
	return Math.min(maxDelta, Math.max(minDelta, deltaBeats));
}

/** Produce NoteEdit list for moving selected notes by deltas. */
export function applyGroupMove(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaPitch: number,
	deltaBeats: number,
	blockBeats: number
): NoteEdit[] {
	const clamped = clampGroupMove(notes, selection, deltaPitch, deltaBeats, blockBeats);
	const edits: NoteEdit[] = [];
	for (const index of selection) {
		const note = notes[index];
		if (note === undefined) continue;
		edits.push({
			note: index,
			pitch: note.pitch + clamped.deltaPitch,
			startBeats: note.startBeats + clamped.deltaBeats,
			lengthBeats: note.lengthBeats,
			velocity: note.velocity
		});
	}
	return edits;
}

/** Produce NoteEdit list for resizing selected notes from the left edge. */
export function applyGroupResizeLeft(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaBeats: number
): NoteEdit[] {
	const clampedDelta = clampGroupResizeLeft(notes, selection, deltaBeats);
	const edits: NoteEdit[] = [];
	for (const index of selection) {
		const note = notes[index];
		if (note === undefined) continue;
		edits.push({
			note: index,
			pitch: note.pitch,
			startBeats: note.startBeats + clampedDelta,
			lengthBeats: note.lengthBeats - clampedDelta,
			velocity: note.velocity
		});
	}
	return edits;
}

/** Produce NoteEdit list for resizing selected notes from the right edge. */
export function applyGroupResizeRight(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaBeats: number,
	blockBeats: number
): NoteEdit[] {
	const clampedDelta = clampGroupResizeRight(notes, selection, deltaBeats, blockBeats);
	const edits: NoteEdit[] = [];
	for (const index of selection) {
		const note = notes[index];
		if (note === undefined) continue;
		edits.push({
			note: index,
			pitch: note.pitch,
			startBeats: note.startBeats,
			lengthBeats: note.lengthBeats + clampedDelta,
			velocity: note.velocity
		});
	}
	return edits;
}

/** Copy selected notes to a clipboard structure relative to earliest start. */
export function copySelection(
	notes: readonly NoteView[],
	selection: Iterable<number>
): NoteClipboard | null {
	const selected = Array.from(selection)
		.map((i) => notes[i])
		.filter((n): n is NoteView => n !== undefined);
	if (selected.length === 0) return null;
	const minStart = Math.min(...selected.map((n) => n.startBeats));
	const maxEnd = Math.max(...selected.map((n) => n.startBeats + n.lengthBeats));
	const clipNotes: ClipboardNote[] = selected.map((n) => ({
		pitch: n.pitch,
		startOffset: n.startBeats - minStart,
		lengthBeats: n.lengthBeats,
		velocity: n.velocity
	}));
	return {
		notes: clipNotes,
		spanBeats: maxEnd - minStart,
		latestEnd: maxEnd
	};
}

/** Paste clipboard notes at target beat or after copied notes end. */
export function pasteClipboard(
	clipboard: NoteClipboard,
	targetBeats: number | null,
	blockBeats: number
): NoteView[] {
	if (clipboard.notes.length === 0) return [];
	const baseStart = targetBeats !== null ? targetBeats : clipboard.latestEnd;
	const clampedBase = Math.max(
		0,
		Math.min(baseStart, Math.max(0, blockBeats - clipboard.spanBeats))
	);
	const result: NoteView[] = [];
	for (const item of clipboard.notes) {
		const startBeats = Math.min(
			Math.max(0, blockBeats - MIN_NOTE_BEATS),
			clampedBase + item.startOffset
		);
		const lengthBeats = clampLength(startBeats, item.lengthBeats, blockBeats);
		result.push({
			pitch: item.pitch,
			startBeats,
			lengthBeats,
			velocity: item.velocity
		});
	}
	return result;
}

/** Duplicate selected notes right after their latest end. */
export function duplicateSelection(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	blockBeats: number
): NoteView[] {
	const selected = Array.from(selection)
		.map((i) => notes[i])
		.filter((n): n is NoteView => n !== undefined);
	if (selected.length === 0) return [];
	const minStart = Math.min(...selected.map((n) => n.startBeats));
	const maxEnd = Math.max(...selected.map((n) => n.startBeats + n.lengthBeats));
	const offset = maxEnd - minStart;
	const result: NoteView[] = [];
	for (const note of selected) {
		const targetStart = note.startBeats + offset;
		const startBeats = Math.min(Math.max(0, blockBeats - MIN_NOTE_BEATS), targetStart);
		const lengthBeats = clampLength(startBeats, note.lengthBeats, blockBeats);
		result.push({
			pitch: note.pitch,
			startBeats,
			lengthBeats,
			velocity: note.velocity
		});
	}
	return result;
}

/** Apply arrow horizontal nudge to selected notes. */
export function arrowNudge(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaBeats: number,
	blockBeats: number
): NoteEdit[] {
	return applyGroupMove(notes, selection, 0, deltaBeats, blockBeats);
}

/** Apply arrow pitch transposition to selected notes. */
export function arrowTranspose(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	semitones: number
): NoteEdit[] {
	return applyGroupMove(notes, selection, semitones, 0, Infinity);
}

/** Apply arrow Shift+Left or Shift+Right resize to selected notes. */
export function arrowResize(
	notes: readonly NoteView[],
	selection: Iterable<number>,
	deltaBeats: number,
	blockBeats: number
): NoteEdit[] {
	return applyGroupResizeRight(notes, selection, deltaBeats, blockBeats);
}
