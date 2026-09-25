<script lang="ts">
	/**
	 * The left sidebar: the file browser, the voice picker and the voice editor,
	 * one visible at a time. All three stay mounted so the browser keeps its place.
	 */

	import EffectsPane from './EffectsPane.svelte';
	import FileBrowser from './FileBrowser.svelte';
	import VstPane from './VstPane.svelte';

	type Tab = 'files' | 'vst' | 'effects';

	const TABS: { id: Tab; label: string }[] = [
		{ id: 'files', label: 'Browser' },
		{ id: 'vst', label: 'VST' },
		{ id: 'effects', label: 'Effects' }
	];

	let tab = $state<Tab>('files');
</script>

<aside>
	<div class="tabs" role="tablist" aria-label="Sidebar panes">
		{#each TABS as entry (entry.id)}
			<button
				class="tab"
				role="tab"
				id="tab-{entry.id}"
				aria-selected={tab === entry.id}
				aria-controls="pane-{entry.id}"
				onclick={() => (tab = entry.id)}
			>
				{entry.label}
			</button>
		{/each}
	</div>

	<div class="panes">
		<div
			class="pane"
			role="tabpanel"
			id="pane-files"
			aria-labelledby="tab-files"
			hidden={tab !== 'files'}
		>
			<FileBrowser />
		</div>
		<div
			class="pane"
			role="tabpanel"
			id="pane-vst"
			aria-labelledby="tab-vst"
			hidden={tab !== 'vst'}
		>
			<VstPane />
		</div>
		<div
			class="pane"
			role="tabpanel"
			id="pane-effects"
			aria-labelledby="tab-effects"
			hidden={tab !== 'effects'}
		>
			<EffectsPane />
		</div>
	</div>
</aside>

<style>
	aside {
		flex: none;
		width: 240px;
		display: flex;
		flex-direction: column;
		min-height: 0;
		background: var(--panel);
		border-right: 1px solid var(--line);
	}

	.tabs {
		display: flex;
		flex: none;
		border-bottom: 1px solid var(--line);
	}

	.tab {
		flex: 1;
		padding: 5px 4px;
		font-size: 9px;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--dim);
		background: transparent;
		border: 0;
		border-bottom: 2px solid transparent;
		border-radius: 0;
	}

	.tab:hover {
		color: var(--text);
	}

	.tab[aria-selected='true'] {
		color: var(--text);
		border-bottom-color: var(--accent);
	}

	.panes {
		flex: 1;
		min-height: 0;
		display: flex;
	}

	.pane {
		flex: 1;
		min-width: 0;
		min-height: 0;
	}

	.pane[hidden] {
		display: none;
	}
</style>
