/**
 * The detail pane store: the reactive copy of `DetailState`, moved only through
 * the pure transitions in `detail.ts`, and saved to `localStorage` whenever the
 * height or the open state changes. Storage may be missing or refuse a write,
 * so every access is guarded and the pane works without it.
 */

import {
	INITIAL,
	STORAGE_KEY,
	openClip,
	restore,
	selectTrack,
	serialize,
	showView,
	toggleView,
	type DetailState,
	type DetailView
} from './detail';

function load(): DetailState {
	try {
		return restore(globalThis.localStorage?.getItem(STORAGE_KEY) ?? null);
	} catch {
		return INITIAL;
	}
}

function save(state: DetailState): void {
	try {
		globalThis.localStorage?.setItem(STORAGE_KEY, serialize(state));
	} catch {
		return;
	}
}

class DetailStore {
	state = $state<DetailState>(load());

	get open(): boolean {
		return this.state.open;
	}

	get view(): DetailView {
		return this.state.view;
	}

	get height(): number {
		return this.state.height;
	}

	/** Show the Clip view, opening the pane. */
	showClip(): void {
		this.apply(openClip(this.state));
	}

	/** Follow a track picked from its header: Devices, if the pane is open. */
	followTrack(): void {
		this.apply(selectTrack(this.state));
	}

	/** Switch between Clip and Devices, or open the pane when hidden. */
	toggle(): void {
		this.apply(toggleView(this.state));
	}

	/** Show one view, opening the pane. */
	show(view: DetailView): void {
		this.apply(showView(this.state, view));
	}

	/** Hide or show the pane. */
	setOpen(open: boolean): void {
		this.apply({ ...this.state, open });
	}

	/** Set the body height; the pane clamps it to the window as it draws. */
	setHeight(height: number): void {
		this.apply({ ...this.state, height });
	}

	private apply(next: DetailState): void {
		const persist = next.open !== this.state.open || next.height !== this.state.height;
		this.state = next;
		if (persist) save(next);
	}
}

export const detail = new DetailStore();
