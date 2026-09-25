/**
 * The geometry of the timeline: one pixel scale that the ruler, the lanes and
 * the playhead all derive from, the snap grid, the bar maths and the drawing
 * maths the lanes share. Every function here is pure.
 */

/** Pixels per beat at rest, and the range the zoom keys move through. */
export const ZOOM_DEFAULT = 32;
export const ZOOM_MIN = 8;
export const ZOOM_MAX = 256;
const ZOOM_STEP = 1.25;

/** Beats a drag snaps to, unless Alt is held. */
export const SNAP_BEATS = 1;
/** Shortest block a resize can leave behind: one sixteenth note. */
export const MIN_LENGTH_BEATS = 0.25;
/** Length a double-clicked block is created with: one bar. */
export const DEFAULT_LENGTH_BEATS = 4;
/** Narrowest a block may be drawn, so a short one stays grabbable. */
export const MIN_BLOCK_PX = 14;

/** Bars the ruler draws even when the song is empty. */
export const MIN_BARS = 8;
/** Narrowest a bar may be before its number is drawn every other bar, or fourth. */
export const MIN_LABEL_PX = 44;

/** Track rows, the ruler strip and the header column, in pixels. */
export const DEFAULT_ROW_HEIGHT = 64;
export const ROW_HEIGHT = 64;
export const MIN_ROW_HEIGHT = 32;
export const MAX_ROW_HEIGHT = 300;
export const RULER_HEIGHT = 56;
export const HEADER_WIDTH = 200;

/** Beats to pixels at a scale. */
export function beatsToPx(beats: number, pxPerBeat: number): number {
	return beats * pxPerBeat;
}

/** Pixels to beats at a scale. */
export function pxToBeats(px: number, pxPerBeat: number): number {
	return px / pxPerBeat;
}

/** Clamp a beat to the grid, or leave it free when Alt holds the drag. */
export function snapBeats(beats: number, free = false): number {
	const snapped = free ? beats : Math.round(beats / SNAP_BEATS) * SNAP_BEATS;
	return Math.max(0, snapped);
}

/** Keep a scale inside the zoom range. */
export function clampZoom(pxPerBeat: number): number {
	return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, pxPerBeat));
}

/** The next scale one zoom press away, clamped. */
export function zoomBy(pxPerBeat: number, direction: number): number {
	return clampZoom(direction > 0 ? pxPerBeat * ZOOM_STEP : pxPerBeat / ZOOM_STEP);
}

/** Where a beat sits: bar and beat counting from one, plus the rest of the beat. */
interface Position {
	bar: number;
	beat: number;
	fraction: number;
}

/** Split an absolute beat into bar, beat and the fractional beat inside it. */
export function position(beats: number, beatsPerBar: number): Position {
	const per = beatsPerBar > 0 ? beatsPerBar : 4;
	const safe = Math.max(0, beats);
	const bar = Math.floor(safe / per);
	const into = safe - bar * per;
	const beat = Math.floor(into);
	return { bar: bar + 1, beat: beat + 1, fraction: into - beat };
}

/** A beat as `bar.beat`, the way the transport reads it. */
export function formatPosition(beats: number, beatsPerBar: number): string {
	const at = position(beats, beatsPerBar);
	return `${at.bar}.${at.beat}`;
}

/** How many bars the ruler has to draw to cover a length, and the engine's count. */
export function barsFor(lengthBeats: number, beatsPerBar: number, reported = 0): number {
	const per = beatsPerBar > 0 ? beatsPerBar : 4;
	return Math.max(MIN_BARS, Math.trunc(reported), Math.ceil(lengthBeats / per));
}

/** Every nth bar gets a number, so the labels never collide at a small zoom. */
export function labelStep(barPx: number): number {
	let step = 1;
	while (barPx * step < MIN_LABEL_PX && step < 64) step *= 2;
	return step;
}

/** Where a lane's material ends, which is where the next block may start. */
export function freeStart(blocks: readonly { startBeats: number; lengthBeats: number }[]): number {
	return blocks.reduce((end, block) => Math.max(end, block.startBeats + block.lengthBeats), 0);
}

/**
 * An SVG polygon of a peak list, mirrored around a centre line. The caller sizes
 * it with `viewBox="0 0 {peaks.length - 1} {height}"` and stretches it.
 */
export function waveformPoints(peaks: readonly number[], height: number): string {
	if (peaks.length === 0) return '';
	const middle = height / 2;
	const rising: string[] = [];
	const falling: string[] = [];
	peaks.forEach((peak, index) => {
		const amplitude = Math.min(1, Math.max(0, peak)) * middle;
		rising.push(`${index},${middle - amplitude}`);
		falling.push(`${index},${middle + amplitude}`);
	});
	return [...rising, ...falling.reverse()].join(' ');
}

/**
 * An SVG polyline of an ADSR envelope. The three times share the width that the
 * sustain plateau leaves, so a long attack visibly eats the rest of the shape.
 */
export function envelopePoints(
	attack: number,
	decay: number,
	sustain: number,
	release: number,
	width: number,
	height: number,
	sustainWidth: number
): string {
	const shape = Math.max(0.001, attack + decay + release);
	const span = Math.max(1, width - sustainWidth);
	const rise = (attack / shape) * span;
	const fall = (decay / shape) * span;
	const tail = (release / shape) * span;
	const held = Math.min(1, Math.max(0, sustain));
	const level = (value: number) => height - Math.min(1, Math.max(0, value)) * height;
	const plateau = rise + fall;
	return [
		`0,${level(0)}`,
		`${rise},${level(1)}`,
		`${plateau},${level(held)}`,
		`${plateau + sustainWidth},${level(held)}`,
		`${plateau + sustainWidth + tail},${level(0)}`
	].join(' ');
}
