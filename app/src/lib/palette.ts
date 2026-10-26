/**
 * The track palette: muted fills the engine picks between by `colorIndex`,
 * drawn from the Kono family of sage, olive, sky, sand and rust. They stay
 * legible on black and apart from each other, and none is the white a
 * selection outline uses.
 */

const TRACK_PALETTE: readonly string[] = [
	'#88A888',
	'#A8B365',
	'#7FA7BF',
	'#C9B27C',
	'#C27A5E',
	'#9C8FB5',
	'#6FA59A',
	'#B58A8A'
];

/** The fill for a track, wrapping if the engine ever hands out an index past the end. */
export function trackColor(colorIndex: number): string {
	const size = TRACK_PALETTE.length;
	const index = ((Math.trunc(colorIndex) % size) + size) % size;
	return TRACK_PALETTE[index];
}
