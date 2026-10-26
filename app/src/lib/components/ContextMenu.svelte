<script lang="ts">
	/**
	 * The window's context menu, mounted once. It follows the WAI-ARIA menu
	 * pattern: focus lands on the first enabled item, the arrows, Home and End
	 * rove through the items, Enter or Space runs one, and Escape or Tab closes
	 * the menu and hands focus back. While it is open it listens on the window in
	 * the capture phase and swallows every key, so the keymap never plays a note
	 * or moves the cursor behind it. Shift+F10 and the ContextMenu key open the
	 * selected block's menu.
	 */

	import { onMount, tick } from 'svelte';

	import { clampToViewport, type MenuEntry, type MenuItem } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';

	let element = $state<HTMLDivElement | null>(null);
	let position = $state({ x: 0, y: 0 });
	let active = $state(-1);

	const open = $derived(menu.state);

	function isItem(entry: MenuEntry): entry is MenuItem {
		return entry.kind === 'item';
	}

	function itemIndices(entries: MenuEntry[]): number[] {
		return entries.flatMap((entry, index) => (isItem(entry) ? [index] : []));
	}

	function focusEntry(index: number): void {
		active = index;
		element?.querySelector<HTMLElement>(`[data-entry="${index}"]`)?.focus();
	}

	function step(direction: number): void {
		if (open === null) return;
		const indices = itemIndices(open.entries);
		if (indices.length === 0) return;
		const at = indices.indexOf(active);
		const next = at === -1 ? (direction > 0 ? 0 : indices.length - 1) : (at + direction + indices.length) % indices.length;
		focusEntry(indices[next]);
	}

	function edge(last: boolean): void {
		if (open === null) return;
		const indices = itemIndices(open.entries);
		if (indices.length > 0) focusEntry(last ? indices[indices.length - 1] : indices[0]);
	}

	function activate(index: number): void {
		const entry = open?.entries[index];
		if (entry === undefined || !isItem(entry) || entry.disabled) return;
		menu.close(true);
		entry.run();
	}

	function openSelectedBlock(): void {
		const block = document.querySelector('[data-block].selected');
		if (!(block instanceof HTMLElement)) return;
		const rect = block.getBoundingClientRect();
		block.dispatchEvent(
			new MouseEvent('contextmenu', {
				bubbles: true,
				cancelable: true,
				clientX: rect.left + Math.min(rect.width / 2, 24),
				clientY: rect.bottom
			})
		);
	}

	function onKeyDown(event: KeyboardEvent): void {
		if (open === null) {
			if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
				event.preventDefault();
				event.stopImmediatePropagation();
				openSelectedBlock();
			}
			return;
		}
		event.preventDefault();
		event.stopImmediatePropagation();
		if (event.key === 'ArrowDown') step(1);
		else if (event.key === 'ArrowUp') step(-1);
		else if (event.key === 'Home') edge(false);
		else if (event.key === 'End') edge(true);
		else if (event.key === 'Enter' || event.key === ' ') activate(active);
		else if (event.key === 'Escape' || event.key === 'Tab') menu.close(true);
	}

	function onPointerDown(event: PointerEvent): void {
		if (open === null) return;
		if (event.target instanceof Node && element?.contains(event.target)) return;
		menu.close();
	}

	function onDismiss(): void {
		menu.close();
	}

	onMount(() => {
		window.addEventListener('keydown', onKeyDown, true);
		window.addEventListener('pointerdown', onPointerDown, true);
		window.addEventListener('blur', onDismiss);
		window.addEventListener('resize', onDismiss);
		window.addEventListener('scroll', onDismiss, true);
		return () => {
			window.removeEventListener('keydown', onKeyDown, true);
			window.removeEventListener('pointerdown', onPointerDown, true);
			window.removeEventListener('blur', onDismiss);
			window.removeEventListener('resize', onDismiss);
			window.removeEventListener('scroll', onDismiss, true);
		};
	});

	$effect(() => {
		const current = open;
		if (current === null) {
			active = -1;
			return;
		}
		position = { x: current.x, y: current.y };
		void tick().then(() => {
			if (element === null || menu.state !== current) return;
			const rect = element.getBoundingClientRect();
			position = clampToViewport(current.x, current.y, rect.width, rect.height, window.innerWidth, window.innerHeight);
			const first = current.entries.findIndex((entry) => isItem(entry) && !entry.disabled);
			const fallback = itemIndices(current.entries)[0] ?? -1;
			const target = first === -1 ? fallback : first;
			if (target === -1) element.focus();
			else focusEntry(target);
		});
	});
</script>

{#if open !== null}
	<div
		class="menu"
		role="menu"
		tabindex="-1"
		aria-orientation="vertical"
		bind:this={element}
		style="left: {position.x}px; top: {position.y}px"
		oncontextmenu={(event) => event.preventDefault()}
	>
		{#each open.entries as entry, index (index)}
			{#if entry.kind === 'separator'}
				<div class="separator" role="separator"></div>
			{:else}
				<button
					type="button"
					class="item"
					class:danger={entry.danger}
					role="menuitem"
					data-entry={index}
					tabindex={index === active ? 0 : -1}
					aria-disabled={entry.disabled ? 'true' : undefined}
					onclick={() => activate(index)}
					onpointermove={() => {
						if (active !== index) focusEntry(index);
					}}
				>
					<span class="label">{entry.label}</span>
					{#if entry.shortcut}
						<span class="shortcut">{entry.shortcut}</span>
					{/if}
				</button>
			{/if}
		{/each}
	</div>
{/if}

<style>
	.menu {
		position: fixed;
		z-index: 100;
		min-width: 168px;
		padding: 3px 0;
		display: flex;
		flex-direction: column;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 0;
		box-shadow: 0 6px 18px rgb(0 0 0 / 0.35);
		font-size: 11px;
		outline: none;
	}

	.item {
		display: flex;
		align-items: center;
		gap: 16px;
		width: 100%;
		padding: 4px 10px;
		border: 0;
		border-radius: 0;
		background: transparent;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: default;
	}

	.item:focus {
		outline: none;
		background: color-mix(in srgb, var(--accent) 30%, transparent);
	}

	.item:focus-visible {
		outline: 1px solid var(--accent);
		outline-offset: -1px;
	}

	.item[aria-disabled='true'] {
		color: var(--dim);
	}

	.item.danger:not([aria-disabled='true']) {
		color: var(--warn);
	}

	.label {
		flex: 1;
		white-space: nowrap;
	}

	.shortcut {
		font-size: 10px;
		color: var(--dim);
		white-space: nowrap;
	}

	.separator {
		height: 1px;
		margin: 3px 0;
		background: var(--line);
	}
</style>
