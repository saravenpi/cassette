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

/** Grid steps the snap picks from, coarsest first, in beats. */
const GRID_STEPS = [2, 1, 1 / 2, 1 / 4, 1 / 8, 1 / 16];
/** Narrowest a grid step may be on screen before the snap picks a coarser one. */
const MIN_GRID_PX = 12;
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

/**
 * The grid step at a zoom: the finest step still at least `MIN_GRID_PX` wide,
 * so zooming in snaps to finer divisions the way other DAWs do.
 */
export function gridStep(pxPerBeat: number): number {
	const fine = GRID_STEPS.filter((step) => step * pxPerBeat >= MIN_GRID_PX);
	return fine[fine.length - 1] ?? GRID_STEPS[0];
}

/** Clamp a beat to a grid of `step` beats, or leave it free when Alt holds the drag. */
export function snapBeats(beats: number, step: number, free = false): number {
	const snapped = free || step <= 0 ? beats : Math.round(beats / step) * step;
	return Math.max(0, snapped);
}

/**
 * A left-edge trim: the new start held between `floor` and one shortest block
 * before `end`, which does not move, and the length that leaves.
 */
export function trimSpan(
	start: number,
	end: number,
	floor: number
): { startBeats: number; lengthBeats: number } {
	const startBeats = Math.min(Math.max(start, floor), end - MIN_LENGTH_BEATS);
	return { startBeats, lengthBeats: end - startBeats };
}

/** Slowest and fastest an audio block may play its material, as the engine clamps it. */
export const MIN_RATE = 0.25;
export const MAX_RATE = 4;
/** Longest block the engine keeps, in beats. */
export const MAX_LENGTH_BEATS = 512;

/**
 * The length a stretch to `wanted` beats ends at: the block's material, which
 * is `length * rate`, held fixed while the rate stays inside the engine's range.
 */
export function stretchLength(length: number, rate: number, wanted: number): number {
	const material = length * rate;
	const span = Math.min(Math.max(wanted, material / MAX_RATE), material / MIN_RATE);
	return Math.min(Math.max(span, MIN_LENGTH_BEATS), MAX_LENGTH_BEATS);
}

/**
 * The stretch of a waveform's x axis a block shows, in peak indices: from the
 * point its offset reaches into the file, for as many seconds of material as
 * its length plays at its rate. Past the file's end the window runs on blank.
 */
export function waveWindow(
	offsetSeconds: number,
	lengthBeats: number,
	bpm: number,
	rate: number,
	seconds: number,
	buckets: number
): { x: number; width: number } {
	if (seconds <= 0 || bpm <= 0 || buckets <= 1) return { x: 0, width: Math.max(1, buckets - 1) };
	const perSecond = (buckets - 1) / seconds;
	const played = ((lengthBeats * 60) / bpm) * rate;
	return { x: offsetSeconds * perSecond, width: Math.max(played * perSecond, 0.001) };
}

/** Where `value` sits between `min` and `max`, as a CSS percentage for a slider's fill. */
export function rangePercent(value: number, min: number, max: number): string {
	const span = max - min;
	const share = span > 0 ? (value - min) / span : 0;
	return `${Math.min(100, Math.max(0, share * 100))}%`;
}

/** Keep a scale inside the zoom range. */
export function clampZoom(pxPerBeat: number): number {
	return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, pxPerBeat));
}

/** How strongly one pixel of wheel travel zooms; a notch of about 100 px is one press. */
const WHEEL_ZOOM_RATE = 0.0022;

/**
 * The scale after a wheel movement of `deltaY` pixels, clamped. It is
 * exponential, so a trackpad's many small deltas and a mouse notch's one big
 * delta zoom alike.
 */
export function zoomByWheel(pxPerBeat: number, deltaY: number): number {
	return clampZoom(pxPerBeat * Math.exp(-deltaY * WHEEL_ZOOM_RATE));
}

/**
 * The scroll offset that keeps the beat under a pointer `x` pixels into the
 * view in place when the scale changes from `from` to `to`.
 */
export function anchoredScroll(scrollLeft: number, x: number, from: number, to: number): number {
	return Math.max(0, ((scrollLeft + x) / from) * to - x);
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

/** A beat as `bar.beat.hundredths`, precise enough to place a cut by eye. */
export function formatPrecise(beats: number, beatsPerBar: number): string {
	const at = position(beats, beatsPerBar);
	const hundredths = Math.min(99, Math.floor(at.fraction * 100));
	return `${at.bar}.${at.beat}.${String(hundredths).padStart(2, '0')}`;
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

/** A row height held inside the allowed range. */
export function clampRowHeight(height: number): number {
	return Math.max(MIN_ROW_HEIGHT, Math.min(MAX_ROW_HEIGHT, height));
}

/** The custom row heights that still belong to a track, for `count` tracks. */
export function pruneHeights(
	heights: Record<number, number>,
	count: number
): Record<number, number> {
	return Object.fromEntries(Object.entries(heights).filter(([index]) => Number(index) < count));
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
