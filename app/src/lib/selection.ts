/**
 * Pure helpers for time selection in the arrangement view.
 */

import type { BeatRange } from './wire';

/** Normalize any two tracks and beat positions into a valid BeatRange. */
export function normalizeRange(
	trackA: number,
	trackB: number,
	beatA: number,
	beatB: number
): BeatRange {
	return {
		firstTrack: Math.min(trackA, trackB),
		lastTrack: Math.max(trackA, trackB),
		startBeats: Math.max(0, Math.min(beatA, beatB)),
		endBeats: Math.max(0, Math.max(beatA, beatB))
	};
}

/** The length of a selection in beats. */
export function rangeLength(range: BeatRange): number {
	return Math.max(0, range.endBeats - range.startBeats);
}

/** The number of tracks spanned by a selection. */
export function rangeTracks(range: BeatRange): number {
	return Math.max(0, range.lastTrack - range.firstTrack + 1);
}

/** Extend a selection to encompass the given track and beat. */
export function extendRange(
	range: BeatRange | null,
	track: number,
	beats: number
): BeatRange {
	const safeBeat = Math.max(0, beats);
	if (range === null) {
		return {
			firstTrack: track,
			lastTrack: track,
			startBeats: safeBeat,
			endBeats: safeBeat
		};
	}
	return {
		firstTrack: Math.min(range.firstTrack, track),
		lastTrack: Math.max(range.lastTrack, track),
		startBeats: Math.min(range.startBeats, safeBeat),
		endBeats: Math.max(range.endBeats, safeBeat)
	};
}

/** Whether a track index falls within the range. */
export function trackInRange(track: number, range: BeatRange): boolean {
	return track >= range.firstTrack && track <= range.lastTrack;
}

/** Whether a beat position falls within the range. */
export function beatInRange(beats: number, range: BeatRange): boolean {
	return beats >= range.startBeats && beats <= range.endBeats;
}

/** Whether a block on a track overlaps the selection range. */
export function blockOverlapsRange(
	track: number,
	startBeats: number,
	lengthBeats: number,
	range: BeatRange
): boolean {
	if (!trackInRange(track, range)) return false;
	const blockEnd = startBeats + lengthBeats;
	return startBeats < range.endBeats && blockEnd > range.startBeats;
}

/** Adjust the start beat of a range, swapping edges if pulled past the end. */
export function resizeRangeStart(range: BeatRange, newStart: number): BeatRange {
	const clamped = Math.max(0, newStart);
	if (clamped <= range.endBeats) {
		return { ...range, startBeats: clamped };
	}
	return { ...range, startBeats: range.endBeats, endBeats: clamped };
}

/** Adjust the end beat of a range, swapping edges if pulled before the start. */
export function resizeRangeEnd(range: BeatRange, newEnd: number): BeatRange {
	const clamped = Math.max(0, newEnd);
	if (clamped >= range.startBeats) {
		return { ...range, endBeats: clamped };
	}
	return { ...range, startBeats: clamped, endBeats: range.startBeats };
}

/** Format a beat duration in bars.beats for readouts. */
export function formatRangeLength(beats: number, beatsPerBar: number): string {
	const per = beatsPerBar > 0 ? beatsPerBar : 4;
	const safe = Math.max(0, beats);
	const bars = Math.floor(safe / per);
	const rem = safe - bars * per;
	const formattedRem = Number.isInteger(rem) ? String(rem) : rem.toFixed(2);
	return `${bars}.${formattedRem}`;
}
