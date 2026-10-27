/**
 * The device chain as data: which effect kinds a track can still take, where a
 * drop lands between the cards, where a dragged card ends up, the queue key of
 * one parameter, and the one-line blurb the effect browser shows. Every
 * function here is pure.
 */

import type { EffectKind, EffectSpecView, EffectView } from './wire';

/** The horizontal extent of one card, in client pixels. */
export interface Span {
	left: number;
	width: number;
}

/** What each built-in effect does, in one line for the effect browser. */
export const EFFECT_BLURBS: Record<EffectKind, string> = {
	eq: 'Cut or boost the lows, mids and highs.',
	filter: 'Low-, high- or band-pass with resonance.',
	saturator: 'Drive into soft clipping for warmth.',
	compressor: 'Even out the level, or duck under another track with the sidechain.',
	limiter: 'Catch peaks so the track never clips.',
	utility: 'Gain, stereo width and pan.',
	chorus: 'Detuned copies that thicken and widen.',
	phaser: 'Sweeping notches for a swirling tone.',
	autoPan: 'Tempo-synced panning or tremolo.',
	delay: 'Tempo-synced echoes with feedback.',
	reverb: 'A Freeverb room with size and damping.',
	bitCrusher: 'Lower the bit depth and sample rate for grit.'
};

/** The most parameters one effect has, so a queue key never collides. */
const PARAM_SLOTS = 8;
/** The most effects one chain holds, likewise. */
const EFFECT_SLOTS = 8;

/** Whether a chain has room for another effect (maximum 8). */
export function canAddEffect(effects: readonly EffectView[]): boolean {
	return effects.length < EFFECT_SLOTS;
}

/** The catalog entries that can be added to a chain, empty once at the limit of 8. */
export function addableKinds(catalog: readonly EffectSpecView[], effects: readonly EffectView[]): EffectSpecView[] {
	if (!canAddEffect(effects)) return [];
	return [...catalog];
}

/** The slot a pointer at `x` drops into: the number of cards whose middle lies left of it. */
export function dropIndex(x: number, spans: readonly Span[]): number {
	return spans.filter((span) => span.left + span.width / 2 < x).length;
}

/**
 * The final index of a card dragged from `from` and dropped into `slot`. A slot
 * counts the gaps before the card is lifted, so dropping right of it lands one
 * lower. Null when the card would not move.
 */
export function moveTarget(from: number, slot: number): number | null {
	const to = slot > from ? slot - 1 : slot;
	return to === from ? null : to;
}

/** The x of the indicator line for a slot, in the same pixels as the spans; null with no cards. */
export function indicatorX(slot: number, spans: readonly Span[], gap: number): number | null {
	if (spans.length === 0) return null;
	if (slot < spans.length) return spans[slot].left - gap / 2;
	const last = spans[spans.length - 1];
	return last.left + last.width + gap / 2;
}

/** A queue key unique to one parameter of one effect on one track. */
export function paramKey(track: number, index: number, param: number): number {
	return (track * EFFECT_SLOTS + index) * PARAM_SLOTS + param;
}

/** The spec of an effect kind in the catalog, or null when the engine does not list it. */
export function specFor(catalog: readonly EffectSpecView[], kind: EffectKind): EffectSpecView | null {
	return catalog.find((spec) => spec.kind === kind) ?? null;
}
