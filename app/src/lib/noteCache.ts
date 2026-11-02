/**
 * The notes the webview already holds, one entry per track. The engine leaves a
 * track's notes out of the snapshot while its `notesRevision` matches the one
 * sent back, so a frame without note edits carries no notes at all.
 */

import type { NoteView, UiState } from './wire';

interface CachedTrack {
	revision: number;
	blocks: NoteView[][];
}

/**
 * Return `previous` when it holds the same data as `next`, else a copy of `next`
 * that reuses every unchanged branch of `previous`. Identical references short
 * circuit, so note arrays served from the cache cost nothing to compare.
 */
export function share<T>(previous: unknown, next: T): T {
	if (previous === next) return next;
	if (typeof next !== 'object' || next === null) return next;
	if (typeof previous !== 'object' || previous === null) return next;
	if (Array.isArray(next)) {
		if (!Array.isArray(previous)) return next;
		const merged = next.map((item, index) => share(previous[index], item));
		const same = merged.length === previous.length && merged.every((item, i) => item === previous[i]);
		return (same ? previous : merged) as T;
	}
	if (Array.isArray(previous)) return next;
	const before = previous as Record<string, unknown>;
	const after = next as Record<string, unknown>;
	const keys = Object.keys(after);
	const merged: Record<string, unknown> = {};
	let same = keys.length === Object.keys(before).length;
	for (const key of keys) {
		merged[key] = share(before[key], after[key]);
		if (merged[key] !== before[key]) same = false;
	}
	return (same ? previous : merged) as T;
}

export class NoteCache {
	private tracks: CachedTrack[] = [];
	private last: UiState | null = null;

	/** The revision held for each track, in track order, for the next request. */
	known(): number[] {
		return this.tracks.map((track) => track.revision);
	}

	/**
	 * Fill in the notes the engine left out, and remember the ones it sent. A
	 * revision changes with any block edit on the track, so block indices line up.
	 */
	merge(state: UiState): UiState {
		const shared = share(this.last, this.fill(state));
		this.last = shared;
		return shared;
	}

	private fill(state: UiState): UiState {
		state.tracks.forEach((track, index) => {
			const held = this.tracks[index];
			if (held !== undefined && held.revision === track.notesRevision) {
				track.blocks.forEach((block, position) => {
					block.notes = held.blocks[position] ?? [];
				});
			} else {
				this.tracks[index] = {
					revision: track.notesRevision,
					blocks: track.blocks.map((block) => block.notes)
				};
			}
		});
		this.tracks.length = state.tracks.length;
		return state;
	}
}
