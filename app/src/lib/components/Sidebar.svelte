<script lang="ts">
	/**
	 * The left sidebar: the file browser, the voice picker and the effect browser,
	 * one visible at a time. All three stay mounted so the browser keeps its place.
	 * The right edge drags to resize the sidebar, and the header button collapses
	 * or reopens it.
	 */

	import { onDestroy } from 'svelte';

	import EffectsPane from './EffectsPane.svelte';
	import FileBrowser from './FileBrowser.svelte';
	import InstrumentsPane from './InstrumentsPane.svelte';

	type Tab = 'files' | 'instruments' | 'effects';

	const TABS: { id: Tab; label: string }[] = [
		{ id: 'files', label: 'Browser' },
		{ id: 'instruments', label: 'Instruments' },
		{ id: 'effects', label: 'Effects' }
	];

	const MIN_WIDTH = 180;
	const MAX_SHARE = 0.45;
	const DEFAULT_WIDTH = 280;
	const RESIZE_STEP = 16;
	const RESIZE_BIG_STEP = 64;
	const STORAGE_KEY = 'cassette.sidebar';

	function clampWidth(w: number, winW: number): number {
		const max = Math.max(MIN_WIDTH, Math.floor(winW * MAX_SHARE));
		return Math.round(Math.min(max, Math.max(MIN_WIDTH, w)));
	}

	function loadSidebar(): { open: boolean; width: number } {
		try {
			const raw = globalThis.localStorage?.getItem(STORAGE_KEY) ?? null;
			if (raw === null) return { open: true, width: DEFAULT_WIDTH };
			const parsed: unknown = JSON.parse(raw);
			if (typeof parsed !== 'object' || parsed === null) {
				return { open: true, width: DEFAULT_WIDTH };
			}
			const rec = parsed as Record<string, unknown>;
			const open = typeof rec.open === 'boolean' ? rec.open : true;
			const width =
				typeof rec.width === 'number' && Number.isFinite(rec.width)
					? Math.max(MIN_WIDTH, rec.width)
					: DEFAULT_WIDTH;
			return { open, width };
		} catch {
			return { open: true, width: DEFAULT_WIDTH };
		}
	}

	function saveSidebar(nextOpen: boolean, nextWidth: number): void {
		try {
			globalThis.localStorage?.setItem(
				STORAGE_KEY,
				JSON.stringify({ open: nextOpen, width: nextWidth })
			);
		} catch {
			return;
		}
	}

	const initial = loadSidebar();
	let tab = $state<Tab>('files');
	let open = $state(initial.open);
	let savedWidth = $state(initial.width);
	let windowWidth = $state(typeof window === 'undefined' ? 1280 : window.innerWidth);
	let resize: { x: number; width: number } | null = null;

	const width = $derived(clampWidth(savedWidth, windowWidth));
	const maxWidth = $derived(Math.max(MIN_WIDTH, Math.floor(windowWidth * MAX_SHARE)));

	onDestroy(() => stopResize());

	function setOpen(next: boolean): void {
		open = next;
		saveSidebar(open, savedWidth);
	}

	function setWidth(next: number): void {
		savedWidth = next;
		saveSidebar(open, savedWidth);
	}

	function selectTab(next: Tab): void {
		tab = next;
		if (!open) setOpen(true);
	}

	function onResizeDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.preventDefault();
		resize = { x: event.clientX, width };
		window.addEventListener('pointermove', onResizeMove);
		window.addEventListener('pointerup', stopResize);
		window.addEventListener('pointercancel', stopResize);
	}

	function onResizeMove(event: PointerEvent): void {
		if (resize === null) return;
		setWidth(clampWidth(resize.width + event.clientX - resize.x, windowWidth));
	}

	function stopResize(): void {
		resize = null;
		window.removeEventListener('pointermove', onResizeMove);
		window.removeEventListener('pointerup', stopResize);
		window.removeEventListener('pointercancel', stopResize);
	}

	function onResizeKey(event: KeyboardEvent): void {
		const step = event.shiftKey ? RESIZE_BIG_STEP : RESIZE_STEP;
		let next: number | null = null;
		if (event.key === 'ArrowRight') next = width + step;
		else if (event.key === 'ArrowLeft') next = width - step;
		else if (event.key === 'Home') next = MIN_WIDTH;
		else if (event.key === 'End') next = maxWidth;
		if (next === null) return;
		event.preventDefault();
		event.stopPropagation();
		setWidth(clampWidth(next, windowWidth));
	}

	function listen<K extends keyof HTMLElementEventMap>(
		type: K,
		handler: (event: HTMLElementEventMap[K]) => void
	): (node: HTMLElement) => () => void {
		return (node) => {
			node.addEventListener(type, handler);
			return () => node.removeEventListener(type, handler);
		};
	}

	function focusable(node: HTMLElement): void {
		node.tabIndex = 0;
	}
</script>

<svelte:window bind:innerWidth={windowWidth} />

<aside class:collapsed={!open} style={open ? `width: ${width}px` : undefined}>
	{#if open}
		<div class="tabs" role="tablist" aria-label="Sidebar panes">
			{#each TABS as entry (entry.id)}
				<button
					type="button"
					class="tab"
					role="tab"
					id="tab-{entry.id}"
					aria-selected={tab === entry.id}
					aria-controls="pane-{entry.id}"
					onclick={() => selectTab(entry.id)}
				>
					{entry.label}
				</button>
			{/each}
			<button
				type="button"
				class="icon toggle-btn"
				aria-expanded={open}
				aria-label="Collapse the sidebar"
				title="Collapse sidebar"
				onclick={() => setOpen(false)}
			>
				◂
			</button>
		</div>
	{:else}
		<div class="rail" role="tablist" aria-label="Sidebar panes">
			<button
				type="button"
				class="icon toggle-btn"
				aria-expanded={open}
				aria-label="Expand the sidebar"
				title="Expand sidebar"
				onclick={() => setOpen(true)}
			>
				▸
			</button>
			{#each TABS as entry (entry.id)}
				<button
					type="button"
					class="rail-tab"
					role="tab"
					id="tab-{entry.id}"
					aria-selected={tab === entry.id}
					aria-controls="pane-{entry.id}"
					onclick={() => selectTab(entry.id)}
				>
					{entry.label}
				</button>
			{/each}
		</div>
	{/if}

	<div class="panes" hidden={!open}>
		<div
			class="pane"
			role="tabpanel"
			id="pane-files"
			aria-labelledby="tab-files"
			hidden={!open || tab !== 'files'}
		>
			<FileBrowser />
		</div>
		<div
			class="pane"
			role="tabpanel"
			id="pane-instruments"
			aria-labelledby="tab-instruments"
			hidden={!open || tab !== 'instruments'}
		>
			<InstrumentsPane />
		</div>
		<div
			class="pane"
			role="tabpanel"
			id="pane-effects"
			aria-labelledby="tab-effects"
			hidden={!open || tab !== 'effects'}
		>
			<EffectsPane />
		</div>
	</div>

	{#if open}
		<div
			class="handle"
			role="separator"
			aria-orientation="vertical"
			aria-label="Resize the sidebar"
			aria-valuemin={MIN_WIDTH}
			aria-valuemax={maxWidth}
			aria-valuenow={width}
			{@attach focusable}
			{@attach listen('pointerdown', onResizeDown)}
			{@attach listen('keydown', onResizeKey)}
		></div>
	{/if}
</aside>

<style>
	aside {
		position: relative;
		flex: none;
		width: 280px;
		display: flex;
		flex-direction: column;
		min-height: 0;
		background: var(--bg);
		border-right: 1px solid var(--border-subtle);
	}

	aside.collapsed {
		width: 28px;
	}

	.tabs {
		display: flex;
		align-items: center;
		flex: none;
		gap: 2px;
		height: 40px;
		padding: 0 6px 0 8px;
		border-bottom: 1px solid var(--border-subtle);
		min-width: 0;
	}

	.tab {
		height: 100%;
		padding: 0 6px;
		font-size: 11px;
		letter-spacing: 0.12em;
		white-space: nowrap;
		color: var(--text-subtle);
		background: transparent;
		box-shadow: inset 0 -2px 0 transparent;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.tab:hover:not(:disabled) {
		color: var(--text);
		filter: none;
	}

	.tab[aria-selected='true'] {
		color: var(--text);
		box-shadow: inset 0 -2px 0 var(--green);
	}

	.icon {
		width: 24px;
		height: 24px;
		padding: 0;
		flex: none;
		background: transparent;
		color: var(--text-subtle);
		font-size: 12px;
		letter-spacing: 0;
	}

	.tabs .toggle-btn {
		margin-left: auto;
	}

	.icon:hover:not(:disabled) {
		filter: none;
		background: var(--surface-hover);
		color: var(--text);
	}

	.rail {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 8px 0;
		gap: 8px;
		flex: 1;
	}

	.rail-tab {
		writing-mode: vertical-rl;
		text-orientation: mixed;
		padding: 8px 4px;
		font-size: 10px;
		letter-spacing: 0.14em;
		color: var(--text-subtle);
		background: transparent;
		box-shadow: inset -2px 0 0 transparent;
	}

	.rail-tab:hover:not(:disabled) {
		color: var(--text);
		filter: none;
	}

	.rail-tab[aria-selected='true'] {
		color: var(--text);
		box-shadow: inset -2px 0 0 var(--green);
	}

	.panes {
		flex: 1;
		min-height: 0;
		display: flex;
	}

	.panes[hidden] {
		display: none;
	}

	.pane {
		flex: 1;
		min-width: 0;
		min-height: 0;
	}

	.pane[hidden] {
		display: none;
	}

	.handle {
		position: absolute;
		top: 0;
		bottom: 0;
		right: -4px;
		z-index: 3;
		width: 7px;
		cursor: ew-resize;
		touch-action: none;
	}

	.handle:hover,
	.handle:focus-visible {
		outline: none;
		background: var(--green-soft);
		box-shadow: inset -1px 0 0 var(--green), inset 1px 0 0 var(--green);
	}
</style>
