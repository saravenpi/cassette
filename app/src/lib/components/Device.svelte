<script lang="ts">
	/**
	 * One device card in the chain: a title bar with the activator, the name, and
	 * the delete button over a body of controls. A double-click on the title bar,
	 * or Enter on its focused name, folds the card into a narrow strip with the
	 * name turned on its side. With the name focused, Backspace or Delete removes
	 * the device and Alt+Left or Alt+Right moves it; those keys stop here, so the
	 * window keymap never deletes a block behind it. A bypassed device dims its
	 * body. Dragging the title bar is reported to the chain, which reorders.
	 */

	import type { Snippet } from 'svelte';

	interface Props {
		name: string;
		folded: boolean;
		enabled?: boolean;
		effect?: boolean;
		dragging?: boolean;
		selected?: boolean;
		onselect?: () => void;
		onfold: () => void;
		ontoggle?: () => void;
		ondelete?: () => void;
		onmove?: (direction: number) => void;
		onmenu?: (event: MouseEvent) => void;
		ongrab?: (event: PointerEvent) => void;
		children: Snippet;
	}

	let {
		name,
		folded,
		enabled = true,
		effect = false,
		dragging = false,
		selected = false,
		onselect,
		onfold,
		ontoggle,
		ondelete,
		onmove,
		onmenu,
		ongrab,
		children
	}: Props = $props();

	function onTitleKeyDown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			event.stopPropagation();
			onfold();
		} else if ((event.key === 'Backspace' || event.key === 'Delete') && ondelete !== undefined) {
			event.preventDefault();
			event.stopPropagation();
			ondelete();
		} else if (event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight') && onmove !== undefined) {
			event.preventDefault();
			event.stopPropagation();
			onmove(event.key === 'ArrowLeft' ? -1 : 1);
		}
	}

	function onTitlePointerDown(event: PointerEvent): void {
		if (event.button !== 0 || ongrab === undefined) return;
		if (event.target instanceof Element && event.target.closest('button') !== null) return;
		ongrab(event);
	}

	function onContextMenu(event: MouseEvent): void {
		if (onmenu === undefined) return;
		event.preventDefault();
		event.stopPropagation();
		onmenu(event);
	}
</script>

<section
	class="device"
	class:folded
	class:bypassed={!enabled}
	class:dragging
	class:selected
	aria-label="{name} device"
	data-effect-card={effect ? '' : undefined}
	onpointerdown={() => onselect?.()}
	onfocusin={() => onselect?.()}
>
	<div
		class="title"
		class:grab={ongrab !== undefined}
		role="presentation"
		data-menu-target={onmenu === undefined ? undefined : ''}
		ondblclick={(event) => {
			if (event.target instanceof Element && event.target.closest('button') !== null) return;
			onfold();
		}}
		onpointerdown={onTitlePointerDown}
		oncontextmenu={onContextMenu}
	>
		{#if ontoggle !== undefined}
			<button
				type="button"
				class="activator"
				aria-pressed={enabled}
				aria-label="{enabled ? 'Bypass' : 'Enable'} {name}"
				title={enabled ? 'On: click to bypass' : 'Bypassed: click to switch on'}
				onclick={ontoggle}
			></button>
		{/if}
		<span
			class="name"
			role="button"
			tabindex="0"
			aria-expanded={!folded}
			aria-label="{name}: Enter folds{ondelete !== undefined ? ', Delete removes, Alt+arrows move' : ''}"
			onkeydown={onTitleKeyDown}
		>
			{name}
		</span>
		{#if ondelete !== undefined && !folded}
			<button type="button" class="close" aria-label="Delete {name}" title="Delete" onclick={ondelete}>
				&times;
			</button>
		{/if}
	</div>
	{#if !folded}
		<div class="body">
			{@render children()}
		</div>
	{/if}
</section>

<style>
	.device {
		flex: none;
		display: flex;
		flex-direction: column;
		min-height: 0;
		height: 100%;
		background: var(--surface);
		box-shadow: inset 0 0 0 1px var(--border-subtle);
	}

	.device.selected {
		box-shadow: inset 0 0 0 1px var(--green);
	}

	.device.dragging {
		opacity: 0.5;
	}

	.title {
		display: flex;
		align-items: center;
		gap: 6px;
		flex: none;
		height: 26px;
		padding: 0 4px 0 6px;
		border-bottom: 1px solid var(--border-subtle);
		user-select: none;
	}

	.title.grab {
		cursor: grab;
	}

	.device.dragging .title {
		cursor: grabbing;
	}

	.activator {
		flex: none;
		width: 12px;
		height: 12px;
		padding: 0;
		background: transparent;
		border: 1px solid var(--border);
	}

	.activator[aria-pressed='true'] {
		background: var(--green);
		border-color: var(--green);
	}

	.activator:hover:not(:disabled) {
		filter: none;
		border-color: var(--text-subtle);
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 10px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text);
	}

	.name:focus-visible {
		outline-offset: 0;
	}

	.close {
		flex: none;
		width: 20px;
		height: 20px;
		padding: 0;
		background: transparent;
		color: var(--text-subtle);
		font-size: 13px;
		letter-spacing: 0;
	}

	.close:hover:not(:disabled) {
		filter: none;
		color: var(--alert);
		background: var(--alert-soft);
	}

	.body {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: 10px 12px;
	}

	.bypassed .body {
		opacity: 0.4;
	}

	.bypassed .name {
		color: var(--text-subtle);
	}

	.folded {
		width: 28px;
	}

	.folded .title {
		flex: 1;
		flex-direction: column;
		height: auto;
		padding: 7px 0;
		border-bottom: 0;
	}

	.folded .name {
		flex: none;
		writing-mode: vertical-rl;
		max-height: 100%;
	}
</style>
