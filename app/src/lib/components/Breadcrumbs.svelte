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
		<span class="sep">/</span>
		<button class="crumb" onclick={() => onOpen(crumb.path)}>{crumb.label}</button>
	{/each}
</nav>

<style>
	.crumbs {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		padding: 4px 6px;
		border-bottom: 1px solid var(--line);
	}

	.crumb {
		padding: 1px 4px;
		font-size: 10px;
		color: var(--dim);
		background: transparent;
		border: 0;
		border-radius: 0;
	}

	.crumb:hover {
		color: var(--text);
		background: var(--panel-2);
	}

	.up {
		font-size: 12px;
	}

	.sep {
		font-size: 10px;
		color: var(--line);
	}
</style>
