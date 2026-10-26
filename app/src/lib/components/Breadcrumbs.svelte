<script lang="ts">
	/**
	 * The sample path above the listing: one crumb per directory, and the step back
	 * up. The parent comes from the engine rather than from string surgery, so a
	 * mount point and a home directory both behave.
	 */

	import { parentOf } from '$lib/api';

	interface Props {
		path: string | null;
		root: string | null;
		onOpen: (path: string) => void;
	}

	let { path, root, onOpen }: Props = $props();

	const atRoot = $derived(path === null || path === root);

	const crumbs = $derived(crumbsOf(path));

	function crumbsOf(where: string | null): { label: string; path: string }[] {
		if (where === null) return [];
		const out: { label: string; path: string }[] = [];
		let built = '';
		for (const part of where.split('/')) {
			if (part === '') continue;
			built = `${built}/${part}`;
			out.push({ label: part, path: built });
		}
		return out;
	}

	async function up(): Promise<void> {
		if (path === null) return;
		const parent = await parentOf(path).catch(() => null);
		if (parent !== null && parent !== path) onOpen(parent);
	}
</script>

<nav class="crumbs" aria-label="Sample path">
	{#if !atRoot}
		<button class="crumb up" aria-label="Go up one directory" onclick={up}>&uarr;</button>
	{/if}
	{#each crumbs as crumb (crumb.path)}
		<span class="sep" aria-hidden="true">&rsaquo;</span>
		<button class="crumb" onclick={() => onOpen(crumb.path)}>{crumb.label}</button>
	{/each}
</nav>

<style>
	.crumbs {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 2px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border-subtle);
	}

	.crumb {
		height: 20px;
		padding: 0 4px;
		font-size: 11px;
		letter-spacing: 0.02em;
		text-transform: none;
		color: var(--text-subtle);
		background: transparent;
	}

	.crumb:hover:not(:disabled) {
		color: var(--text);
		background: var(--surface-hover);
		filter: none;
	}

	.up {
		width: 20px;
		padding: 0;
	}

	.sep {
		font-size: 11px;
		color: var(--dim);
	}
</style>
