/**
 * The one context menu the window shows at a time: where it was opened, what it
 * offers, and which element gets focus back when it closes.
 */

import type { MenuEntry } from './menu';

/** An open menu. */
export interface OpenMenu {
	x: number;
	y: number;
	entries: MenuEntry[];
	returnFocus: HTMLElement | null;
}

class MenuController {
	state = $state<OpenMenu | null>(null);

	/** Show `entries` at a viewport point, replacing any menu already open. */
	open(x: number, y: number, entries: MenuEntry[], returnFocus?: HTMLElement | null): void {
		const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		this.state = { x, y, entries, returnFocus: returnFocus ?? focused };
	}

	/** Hide the menu, and hand focus back to its opener when `restore` is set. */
	close(restore = false): void {
		const open = this.state;
		if (open === null) return;
		this.state = null;
		if (restore && open.returnFocus?.isConnected) open.returnFocus.focus();
	}
}

export const menu = new MenuController();
