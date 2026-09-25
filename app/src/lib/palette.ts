/**
 * The track palette: eight muted fills the engine picks between by `colorIndex`.
 * They stay distinguishable from each other while none of them competes with the
 * accent blue, so a selection ring and a block fill never read as the same thing.
 */

const TRACK_PALETTE: readonly string[] = [
	'#84B7BD',
	'#A8B365',
	'#D9AF67',
	'#E55B3F',
	'#947164'
];

/** The fill for a track, wrapping if the engine ever hands out an index past the end. */
export function trackColor(colorIndex: number): string {
	const size = TRACK_PALETTE.length;
	const index = ((Math.trunc(colorIndex) % size) + size) % size;
	return TRACK_PALETTE[index];
}
