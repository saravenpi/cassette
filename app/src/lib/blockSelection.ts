/**
 * Pure helpers for the multi-block selection: which blocks the snapshot marks
 * selected, and where a group drag may put them. The engine re-checks every
 * move, so these only shape the preview.
 */

import type { BlockView, TrackView } from './wire';

/** One selected block, by track and lane index, with the view it had. */
export interface GroupMember {
	track: number;
	block: number;
	view: BlockView;
}

/** Where a group drag would put its blocks. */
export interface GroupMove {
	deltaBeats: number;
	deltaTracks: number;
	invalid: boolean;
}

/** Every block the snapshot marks selected, in track then lane order. */
export function selectedBlocks(tracks: TrackView[]): GroupMember[] {
	const out: GroupMember[] = [];
	tracks.forEach((track, trackIndex) => {
		track.blocks.forEach((view, block) => {
			if (view.selected) out.push({ track: trackIndex, block, view });
		});
	});
	return out;
}

/**
 * The move a group drag asks for when its grabbed block, which started at
 * `originBeats`, is wanted at `wantedBeats` and `deltaTracks` lanes away. The
 * beats stop at the top of the song; the move is invalid when a block would
 * leave the tracks or land on a track of the other kind.
 */
export function groupMove(
	members: GroupMember[],
	tracks: TrackView[],
	originBeats: number,
	wantedBeats: number,
	deltaTracks: number
): GroupMove {
	const earliest = Math.min(...members.map((member) => member.view.startBeats));
	const deltaBeats = Math.max(wantedBeats - originBeats, 0 - earliest);
	const invalid = members.some((member) => {
		const target = tracks[member.track + deltaTracks];
		if (target === undefined) return true;
		return deltaTracks !== 0 && target.kind !== member.view.kind;
	});
	return { deltaBeats, deltaTracks, invalid };
}
