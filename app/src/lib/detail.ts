/**
 * The detail pane's state as pure transitions: which view it shows, whether it
 * is open, how tall it is, and how that is saved for the next session. The
 * store in `detail.svelte.ts` applies these and persists the result.
 */

/** The two views the detail pane switches between. */
export type DetailView = 'clip' | 'devices';

/** The detail pane: open or hidden, its view, and its body height in pixels. */
export interface DetailState {
	open: boolean;
	view: DetailView;
	height: number;
}

/** Shortest the pane may be, and the share of the window it may take at most. */
export const MIN_HEIGHT = 160;
export const MAX_SHARE = 0.6;
export const DEFAULT_HEIGHT = 280;
/** Pixels one arrow press, or one Shift+arrow press, resizes the pane by. */
export const RESIZE_STEP = 16;
export const RESIZE_BIG_STEP = 64;

/** The key the pane saves under in `localStorage`. */
export const STORAGE_KEY = 'cassette.detail';

/** Where a fresh session starts: open, on the selected track's devices. */
export const INITIAL: DetailState = { open: true, view: 'devices', height: DEFAULT_HEIGHT };

/** A height held between `MIN_HEIGHT` and `MAX_SHARE` of the window. */
export function clampHeight(height: number, windowHeight: number): number {
	const max = Math.max(MIN_HEIGHT, Math.floor(windowHeight * MAX_SHARE));
	return Math.round(Math.min(max, Math.max(MIN_HEIGHT, height)));
}

/** Opening a block in the piano roll shows the Clip view, opening the pane if hidden. */
export function openClip(state: DetailState): DetailState {
	return { ...state, open: true, view: 'clip' };
}

/** Selecting a track from its header shows its devices, when the pane is open. */
export function selectTrack(state: DetailState): DetailState {
	return state.open ? { ...state, view: 'devices' } : state;
}

/** Shift+Tab: switch views while open; open the pane on its last view while hidden. */
export function toggleView(state: DetailState): DetailState {
	if (!state.open) return { ...state, open: true };
	return { ...state, view: state.view === 'clip' ? 'devices' : 'clip' };
}

/** Pick a view from the header, which also opens the pane. */
export function showView(state: DetailState, view: DetailView): DetailState {
	return { ...state, open: true, view };
}

/** The saved form: the height and whether the pane is open. */
export function serialize(state: DetailState): string {
	return JSON.stringify({ open: state.open, height: state.height });
}

/** A saved form read back over `INITIAL`, ignoring anything malformed. */
export function restore(raw: string | null): DetailState {
	if (raw === null) return INITIAL;
	try {
		const parsed: unknown = JSON.parse(raw);
		if (typeof parsed !== 'object' || parsed === null) return INITIAL;
		const record = parsed as Record<string, unknown>;
		const open = typeof record.open === 'boolean' ? record.open : INITIAL.open;
		const height =
			typeof record.height === 'number' && Number.isFinite(record.height)
				? Math.max(MIN_HEIGHT, record.height)
				: INITIAL.height;
		return { ...INITIAL, open, height };
	} catch {
		return INITIAL;
	}
}
