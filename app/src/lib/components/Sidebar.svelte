<script lang="ts">
	/**
	 * The left sidebar: the file browser, the voice picker and the effect browser,
	 * one visible at a time. All three stay mounted so the browser keeps its place.
	 */

	import EffectsPane from './EffectsPane.svelte';
	import FileBrowser from './FileBrowser.svelte';
	import InstrumentsPane from './InstrumentsPane.svelte';

	type Tab = 'files' | 'instruments' | 'effects';

	const TABS: { id: Tab; label: string }[] = [
		{ id: 'files', label: 'Browser' },
		{ id: 'instruments', label: 'Instruments' },
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
			id="pane-instruments"
			aria-labelledby="tab-instruments"
			hidden={tab !== 'instruments'}
		>
			<InstrumentsPane />
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
		width: 280px;
		display: flex;
		flex-direction: column;
		min-height: 0;
		background: var(--bg);
		border-right: 1px solid var(--border-subtle);
	}

	.tabs {
		display: flex;
		flex: none;
		gap: 4px;
		height: 40px;
		padding: 0 12px;
		border-bottom: 1px solid var(--border-subtle);
	}

	.tab {
		height: 100%;
		padding: 0 8px;
		font-size: 11px;
		letter-spacing: 0.14em;
		white-space: nowrap;
		color: var(--text-subtle);
		background: transparent;
		box-shadow: inset 0 -2px 0 transparent;
	}

	.tab:hover:not(:disabled) {
		color: var(--text);
		filter: none;
	}

	.tab[aria-selected='true'] {
		color: var(--text);
		box-shadow: inset 0 -2px 0 var(--green);
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
