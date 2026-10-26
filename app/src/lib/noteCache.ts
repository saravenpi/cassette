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

export class NoteCache {
	private tracks: CachedTrack[] = [];

	/** The revision held for each track, in track order, for the next request. */
	known(): number[] {
		return this.tracks.map((track) => track.revision);
	}

	/**
	 * Fill in the notes the engine left out, and remember the ones it sent. A
	 * revision changes with any block edit on the track, so block indices line up.
	 */
	merge(state: UiState): UiState {
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
