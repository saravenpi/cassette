/**
 * The right-click menus as data: what a block, a lane, a track header, a device
 * and the metronome button offer, and where a menu may sit on screen. Every builder is pure apart from the
 * `run` closures, which only call the host they are given, so the lists can be
 * tested without a webview or an engine.
 */

import { DEFAULT_LENGTH_BEATS, MIN_LENGTH_BEATS, pxToBeats, snapBeats } from './geometry';
import type { BlockView, EngineEvent, TrackKind, TrackView } from './wire';

/** One choice in a menu. */
export interface MenuItem {
	kind: 'item';
	label: string;
	shortcut?: string;
	disabled?: boolean;
	danger?: boolean;
	checked?: boolean;
	run: () => void;
}

/** A rule between two groups of choices. */
export interface MenuSeparator {
	kind: 'separator';
}

/** A label over a group of choices. */
export interface MenuHeading {
	kind: 'heading';
	label: string;
}

export type MenuEntry = MenuItem | MenuSeparator | MenuHeading;

/** What a menu's choices need from the session. */
export interface MenuHost {
	request(event: EngineEvent): Promise<boolean>;
	openRoll(track: number, block: number): void;
}

/** Rates this close to one count as unstretched. */
const UNIT_RATE_EPSILON = 0.001;
/** Gap a menu keeps from the edge of the window, in pixels. */
const VIEWPORT_MARGIN = 4;

function item(label: string, run: () => void, extra: Partial<MenuItem> = {}): MenuItem {
	return { kind: 'item', label, run, ...extra };
}

const separator: MenuSeparator = { kind: 'separator' };

/** Send `first`, then `second` once the engine has taken the first. */
function chain(host: MenuHost, first: EngineEvent, second: EngineEvent): void {
	void host.request(first).then((accepted) => {
		if (accepted) void host.request(second);
	});
}

/** The absolute beat under a pointer `offsetPx` pixels into a block. */
export function beatUnder(startBeats: number, offsetPx: number, pxPerBeat: number): number {
	return startBeats + Math.max(0, pxToBeats(offsetPx, pxPerBeat));
}

/**
 * Where a cut at `beats` lands: on the grid when the grid line leaves both
 * halves long enough, otherwise exactly under the pointer, and null when even
 * that would leave a sliver. `free` drops the grid, the way Alt does.
 */
export function splitPoint(
	block: BlockView,
	beats: number,
	step: number,
	free = false
): number | null {
	const first = block.startBeats + MIN_LENGTH_BEATS;
	const last = block.startBeats + block.lengthBeats - MIN_LENGTH_BEATS;
	const inside = (at: number) => at >= first && at <= last;
	const snapped = snapBeats(beats, step, free);
	if (inside(snapped)) return snapped;
	return inside(beats) ? beats : null;
}

/**
 * The beat the hover line marks for a pointer at `beats`: over a block, the
 * point a split there would cut at; elsewhere, the grid line a click lands on.
 */
export function markAt(
	block: BlockView | undefined,
	beats: number,
	step: number,
	free = false
): number {
	if (block === undefined) return snapBeats(beats, step, free);
	return splitPoint(block, beats, step, free) ?? beats;
}

/** Keep a `width` by `height` menu opened at `x`, `y` inside the window. */
export function clampToViewport(
	x: number,
	y: number,
	width: number,
	height: number,
	viewportWidth: number,
	viewportHeight: number
): { x: number; y: number } {
	const maxX = Math.max(VIEWPORT_MARGIN, viewportWidth - width - VIEWPORT_MARGIN);
	const maxY = Math.max(VIEWPORT_MARGIN, viewportHeight - height - VIEWPORT_MARGIN);
	return {
		x: Math.min(Math.max(VIEWPORT_MARGIN, x), maxX),
		y: Math.min(Math.max(VIEWPORT_MARGIN, y), maxY)
	};
}

/** The choices for a block right-clicked at the absolute beat `atBeats`. */
export function blockMenu(
	host: MenuHost,
	track: number,
	index: number,
	block: BlockView,
	atBeats: number,
	step: number,
	free = false
): MenuEntry[] {
	const at = { track, block: index };
	const cut = splitPoint(block, atBeats, step, free);
	const entries: MenuEntry[] = [
		item(
			'Split here',
			() => void host.request({ splitBlock: { ...at, atBeats: cut ?? atBeats } }),
			{
				shortcut: 'B',
				disabled: cut === null
			}
		),
		item('Duplicate', () => void host.request({ duplicateBlock: at }), {
			shortcut: 'Ctrl+D'
		}),
		item('Rename', () => chain(host, { selectBlock: at }, 'renameBlock'), {
			shortcut: 'R'
		})
	];
	if (block.kind === 'midi') {
		entries.push(
			item('Open in piano roll', () => host.openRoll(track, index), {
				shortcut: 'P'
			})
		);
	} else {
		entries.push(
			item(
				'Reset stretch',
				() =>
					void host.request({
						stretchBlock: {
							...at,
							lengthBeats: block.lengthBeats * block.rate
						}
					}),
				{ disabled: Math.abs(block.rate - 1) < UNIT_RATE_EPSILON }
			)
		);
	}
	entries.push(
		item(
			'Loop this block',
			() =>
				void host.request({
					setLoopRegion: {
						startBeats: block.startBeats,
						endBeats: block.startBeats + block.lengthBeats
					}
				})
		),
		separator,
		item('Delete', () => void host.request({ deleteBlockAt: at }), {
			shortcut: 'Del',
			danger: true
		})
	);
	return entries;
}

/** The choices for an empty stretch of a lane right-clicked at `beats`. */
export function laneMenu(
	host: MenuHost,
	track: number,
	kind: TrackKind,
	beats: number,
	beatsPerBar: number,
	step: number
): MenuEntry[] {
	const start = snapBeats(beats, step);
	return [
		item(
			'New block here',
			() =>
				void host.request({
					createBlock: {
						track,
						startBeats: start,
						lengthBeats: DEFAULT_LENGTH_BEATS
					}
				}),
			kind === 'midi' ? {} : { disabled: true, shortcut: 'MIDI only' }
		),
		item('Move playhead here', () => void host.request({ seek: { beats: Math.max(0, beats) } })),
		item(
			'Loop from here',
			() =>
				void host.request({
					setLoopRegion: { startBeats: start, endBeats: start + beatsPerBar }
				})
		)
	];
}

/** The choices for a track header. `mix` sends the strip with new toggles. */
export function trackMenu(
	host: MenuHost,
	index: number,
	track: TrackView,
	mix: (muted: boolean, solo: boolean) => void
): MenuEntry[] {
	const select: EngineEvent = { selectTrack: { track: index } };
	return [
		item(track.muted ? 'Unmute' : 'Mute', () => mix(!track.muted, track.solo), {
			shortcut: 'M'
		}),
		item(track.solo ? 'Unsolo' : 'Solo', () => mix(track.muted, !track.solo), {
			shortcut: 'O'
		}),
		separator,
		item('Change kind', () => chain(host, select, 'cycleTrackKind'), {
			shortcut: 'C'
		}),
		item('Add block at end', () => chain(host, select, 'addBlock'), {
			shortcut: 'N'
		})
	];
}

/** Bars of count-in the metronome menu offers. */
export const COUNT_IN_BARS = [0, 1, 2, 4];

/** Record quantize grids the metronome menu offers, in beats; 0 is off. */
export const RECORD_GRIDS: { label: string; beats: number }[] = [
	{ label: 'Off', beats: 0 },
	{ label: '1/4', beats: 1 },
	{ label: '1/8', beats: 0.5 },
	{ label: '1/16', beats: 0.25 }
];

function countInLabel(bars: number): string {
	if (bars === 0) return 'None';
	return bars === 1 ? '1 bar' : `${bars} bars`;
}

/** The metronome menu: the count-in and the record quantize, each checked at its current value. */
export function metronomeMenu(host: MenuHost, countInBars: number, recordQuantizeBeats: number): MenuEntry[] {
	return [
		{ kind: 'heading', label: 'Count-in' },
		...COUNT_IN_BARS.map((bars) =>
			item(countInLabel(bars), () => void host.request({ setCountIn: { bars } }), {
				checked: bars === countInBars
			})
		),
		separator,
		{ kind: 'heading', label: 'Record quantize' },
		...RECORD_GRIDS.map((grid) =>
			item(grid.label, () => void host.request({ setRecordQuantize: { gridBeats: grid.beats } }), {
				checked: Math.abs(grid.beats - recordQuantizeBeats) < 1e-6
			})
		)
	];
}

/** The choices for a device in a chain of `count` effects. `fold` flips its fold. */
export function deviceMenu(
	host: MenuHost,
	track: number,
	index: number,
	count: number,
	folded: boolean,
	fold: () => void
): MenuEntry[] {
	return [
		item('Move left', () => void host.request({ moveEffect: { track, from: index, to: index - 1 } }), {
			shortcut: 'Alt+Left',
			disabled: index === 0
		}),
		item('Move right', () => void host.request({ moveEffect: { track, from: index, to: index + 1 } }), {
			shortcut: 'Alt+Right',
			disabled: index >= count - 1
		}),
		item(folded ? 'Unfold' : 'Fold', fold),
		separator,
		item('Delete', () => void host.request({ removeEffect: { track, index } }), {
			shortcut: 'Del',
			danger: true
		})
	];
}
