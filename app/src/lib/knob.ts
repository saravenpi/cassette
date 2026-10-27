/**
 * The arithmetic behind a device dial: a parameter's value mapped to a 0..1
 * position (along a logarithmic scale when the spec asks for one), the steps
 * a drag, a key or a wheel notch moves it by, and how its value reads with its
 * unit. Every function here is pure.
 */

import type { ParamSpecView } from './wire';

/** Pixels of vertical drag that sweep a dial over its whole range. */
export const DRAG_RANGE_PX = 200;
/** How much finer a drag moves while Shift is held. */
export const FINE_FACTOR = 0.1;
/** Positions one key press, one fine press and one Page key move a dial by. */
export const STEP = 0.01;
export const FINE_STEP = 0.001;
export const BIG_STEP = 0.1;
/** The beat grid a tempo-synced time snaps to: one sixty-fourth note. */
export const BEAT_GRID = 1 / 16;

/** How far one press moves a dial. */
export type StepSize = 'fine' | 'normal' | 'big';

/** The slice of a parameter spec the dial math reads. */
export type DialSpec = Pick<ParamSpecView, 'min' | 'max' | 'unit' | 'log' | 'choices'>;

function clamp(value: number, low: number, high: number): number {
	return Math.min(high, Math.max(low, value));
}

function isLog(spec: DialSpec): boolean {
	return spec.log && spec.min > 0 && spec.max > spec.min;
}

/** A value held inside the spec's range, on whole choices and on the beat grid. */
export function clampParam(value: number, spec: DialSpec): number {
	const held = clamp(value, spec.min, spec.max);
	if (spec.unit === 'choice') return Math.round(held);
	if (spec.unit === 'beats') return clamp(Math.round(held / BEAT_GRID) * BEAT_GRID, spec.min, spec.max);
	return held;
}

/** Where a value sits along the dial, from 0 at the minimum to 1 at the maximum. */
export function toNormal(value: number, spec: DialSpec): number {
	if (spec.max <= spec.min) return 0;
	const held = clamp(value, spec.min, spec.max);
	if (isLog(spec)) return Math.log(held / spec.min) / Math.log(spec.max / spec.min);
	return (held - spec.min) / (spec.max - spec.min);
}

/** The value at a dial position, the inverse of `toNormal`, clamped like `clampParam`. */
export function fromNormal(position: number, spec: DialSpec): number {
	const t = clamp(position, 0, 1);
	const raw = isLog(spec)
		? spec.min * Math.pow(spec.max / spec.min, t)
		: spec.min + t * (spec.max - spec.min);
	return clampParam(raw, spec);
}

/** The value a drag reaches: up raises it, and `DRAG_RANGE_PX` covers the range. */
export function dragParam(startValue: number, spec: DialSpec, dyPx: number, fine = false): number {
	const scale = fine ? FINE_FACTOR : 1;
	return fromNormal(toNormal(startValue, spec) - (dyPx / DRAG_RANGE_PX) * scale, spec);
}

/**
 * The value `direction` presses away. A choice moves one choice and a beat time
 * one grid cell (four on a big step), so a press never rounds back to itself.
 */
export function stepParam(value: number, spec: DialSpec, direction: number, size: StepSize = 'normal'): number {
	const sign = Math.sign(direction);
	if (spec.unit === 'choice') return clampParam(Math.round(value) + sign, spec);
	if (spec.unit === 'beats') {
		const cells = size === 'big' ? 4 : 1;
		return clampParam(value + sign * cells * BEAT_GRID, spec);
	}
	const amount = size === 'fine' ? FINE_STEP : size === 'big' ? BIG_STEP : STEP;
	return fromNormal(toNormal(value, spec) + sign * amount, spec);
}

function trim(text: string): string {
	return text.includes('.') ? text.replace(/\.?0+$/, '') : text;
}

function gcd(a: number, b: number): number {
	return b === 0 ? a : gcd(b, a % b);
}

/** A beat count as a musical fraction (`1/8`, `3/4`, `1 1/2`), or null off the grid. */
export function beatFraction(beats: number): string | null {
	const cells = Math.round(beats / BEAT_GRID);
	if (Math.abs(cells * BEAT_GRID - beats) > 1e-6 || cells <= 0) return null;
	const denominator = Math.round(1 / BEAT_GRID);
	const whole = Math.floor(cells / denominator);
	const rest = cells % denominator;
	if (rest === 0) return `${whole}`;
	const divisor = gcd(rest, denominator);
	const fraction = `${rest / divisor}/${denominator / divisor}`;
	return whole === 0 ? fraction : `${whole} ${fraction}`;
}

/** How a value reads under its dial, with its unit. */
export function formatParam(value: number, spec: DialSpec): string {
	switch (spec.unit) {
		case 'decibels': {
			const text = value.toFixed(1);
			return `${Number(text) > 0 ? '+' : ''}${text === '-0.0' ? '0.0' : text} dB`;
		}
		case 'hertz':
			return value >= 1000 ? `${trim((value / 1000).toFixed(2))} kHz` : `${Math.round(value)} Hz`;
		case 'milliseconds':
			return value < 10 ? `${trim(value.toFixed(1))} ms` : `${Math.round(value)} ms`;
		case 'beats': {
			const fraction = beatFraction(value);
			if (fraction === null) return `${trim(value.toFixed(2))} beats`;
			if (fraction === '1') return '1 beat';
			return fraction.includes('/') ? fraction : `${fraction} beats`;
		}
		case 'ratio':
			return `${trim(value.toFixed(1))}:1`;
		case 'percent':
			return `${Math.round(value * 100)}%`;
		case 'choice':
			return spec.choices[Math.round(value)] ?? '';
	}
}

/** What a screen reader says for a value: `formatParam`, with a fraction's unit spelled out. */
export function speakParam(value: number, spec: DialSpec): string {
	const text = formatParam(value, spec);
	if (spec.unit === 'beats' && !text.endsWith('beat') && !text.endsWith('beats')) {
		return `${text} ${value <= 1 ? 'beat' : 'beats'}`;
	}
	return text;
}

/** A point on a dial's circle, with 0 degrees at twelve o'clock, turning clockwise. */
function polar(cx: number, cy: number, radius: number, degrees: number): { x: number; y: number } {
	const radians = ((degrees - 90) * Math.PI) / 180;
	return { x: cx + radius * Math.cos(radians), y: cy + radius * Math.sin(radians) };
}

/** Degrees a dial sweeps, and where the sweep starts (seven-thirty, clockwise). */
export const SWEEP_DEGREES = 270;
export const SWEEP_START = -135;

/** An SVG arc path along the dial from position `from` to position `to`, both 0..1. */
export function arcPath(cx: number, cy: number, radius: number, from: number, to: number): string {
	const start = SWEEP_START + clamp(from, 0, 1) * SWEEP_DEGREES;
	const end = SWEEP_START + clamp(to, 0, 1) * SWEEP_DEGREES;
	if (end - start < 0.01) return '';
	const a = polar(cx, cy, radius, start);
	const b = polar(cx, cy, radius, end);
	const large = end - start > 180 ? 1 : 0;
	return `M ${a.x.toFixed(2)} ${a.y.toFixed(2)} A ${radius} ${radius} 0 ${large} 1 ${b.x.toFixed(2)} ${b.y.toFixed(2)}`;
}

/** The end of the dial's pointer line at a position. */
export function pointerAt(cx: number, cy: number, radius: number, position: number): { x: number; y: number } {
	return polar(cx, cy, radius, SWEEP_START + clamp(position, 0, 1) * SWEEP_DEGREES);
}
