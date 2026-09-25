/**
 * The track palette: eight muted fills the engine picks between by `colorIndex`.
 * They stay distinguishable from each other while none of them competes with the
 * accent blue, so a selection ring and a block fill never read as the same thing.
 */

const TRACK_PALETTE: readonly string[] = [
	'#4f8f8a',
	'#6f9f5f',
	'#a8a04f',
	'#c08a4a',
	'#c0665a',
	'#a25f8f',
	'#7d6bb0',
	'#6b7a8f'
];

/** The fill for a track, wrapping if the engine ever hands out an index past the end. */
export function trackColor(colorIndex: number): string {
	const size = TRACK_PALETTE.length;
	const index = ((Math.trunc(colorIndex) % size) + size) % size;
	return TRACK_PALETTE[index];
}
