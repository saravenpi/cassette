<script lang="ts">
	/**
	 * The file browser: a real directory listing over `list_samples`, a breadcrumb
	 * with a way up, and the two routes a sample takes to a track, a pointer drag
	 * onto a lane and a double-click that lands it on the selected track.
	 */

	import { onMount } from 'svelte';

	import { listSamples, samplesRoot } from '$lib/api';
	import { freeStart, snapBeats } from '$lib/geometry';
	import { SampleDrag } from '$lib/sampleDrag.svelte';
	import { session } from '$lib/session.svelte';
	import type { SampleEntry } from '$lib/wire';
	import Breadcrumbs from './Breadcrumbs.svelte';
	import SamplePreview from './SamplePreview.svelte';

	let dir = $state<string | null>(null);
	let entries = $state<SampleEntry[]>([]);
	let failure = $state<string | null>(null);
	let loading = $state(false);
	let selected = $state<SampleEntry | null>(null);
	let showAll = $state(false);

	const visibleEntries = $derived(
		showAll ? entries : entries.filter((entry) => entry.kind !== 'other')
	);
	const hiddenCount = $derived(entries.filter((entry) => entry.kind === 'other').length);

	const drag = new SampleDrag({
		zoom: () => session.zoom,
		drop: (entry, track, startBeats) => {
			session.dispatch({ importSample: { track, path: entry.path, startBeats } });
		}
	});

	onMount(() => {
		void open(null);
	});

	function reason(error: unknown): string {
		return error instanceof Error ? error.message : String(error);
	}

	function sizeLabel(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	async function open(target: string | null): Promise<void> {
		loading = true;
		try {
			const listing = await listSamples(target);
			entries = listing;
			failure = null;
			dir = target ?? (await samplesRoot().catch(() => null));
		} catch (error) {
			entries = [];
			failure = `${target ?? 'the samples root'} could not be read: ${reason(error)}`;
		} finally {
			loading = false;
		}
	}

	function importToSelected(entry: SampleEntry): void {
		const snapshot = session.snapshot;
		const track = snapshot?.cursor.track ?? 0;
		const blocks = snapshot?.tracks[track]?.blocks ?? [];
		session.dispatch({
			importSample: { track, path: entry.path, startBeats: snapBeats(freeStart(blocks)) }
		});
	}
</script>

<div class="browser">
	<Breadcrumbs path={dir} onOpen={(path) => open(path)} />

	{#if loading}
		<p class="note">reading…</p>
	{:else if failure !== null}
		<p class="note warn">{failure}</p>
	{:else if entries.length === 0}
		<p class="note">this directory is empty</p>
	{:else if visibleEntries.length === 0}
		<p class="note">no audio files</p>
	{:else}
		<ul class="list">
			{#each visibleEntries as entry (entry.path)}
				<li>
					{#if entry.kind === 'dir'}
						<button class="row" onclick={() => open(entry.path)}>
							<span class="name">{entry.name}</span>
							<span class="meta">dir</span>
						</button>
					{:else if entry.kind === 'wav' || entry.kind === 'audio'}
						<button
							class="row wav"
							class:picked={selected?.path === entry.path}
							title="Drag onto a track, or double-click to import into the selected track"
							onpointerdown={(event) => drag.begin(event, entry)}
							onclick={() => (selected = entry)}
							ondblclick={() => importToSelected(entry)}
						>
							<span class="name">{entry.name}</span>
							<span class="meta">{sizeLabel(entry.size)}</span>
						</button>
					{:else}
						<div class="row other">
							<span class="name">{entry.name}</span>
							<span class="meta">not audio</span>
						</div>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	{#if hiddenCount > 0}
		<div class="filter-footer">
			<button class="toggle-hidden" onclick={() => (showAll = !showAll)}>
				{showAll ? 'hide non-audio' : `${hiddenCount} non-audio hidden`}
			</button>
		</div>
	{/if}

	<SamplePreview entry={selected} />
</div>

{#if drag.entry !== null && drag.ghost !== null}
	<div class="ghost" style="left: {drag.ghost.x + 14}px; top: {drag.ghost.y + 14}px">
		{drag.entry.name}
	</div>
{/if}

<style>
	.browser {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.list {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		margin: 0;
		padding: 2px 0;
		list-style: none;
	}

	.row {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 2px 8px;
		font-size: 11px;
		text-align: left;
		color: var(--text);
		background: transparent;
		border: 0;
		border-radius: 0;
	}

	.row:hover {
		background: var(--panel-2);
	}

	.row.wav {
		cursor: grab;
	}

	.row.picked {
		background: var(--panel-2);
		box-shadow: inset 2px 0 0 var(--accent);
	}

	.row.other {
		color: var(--dim);
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		margin-left: auto;
		flex: none;
		font-size: 10px;
		color: var(--dim);
		font-variant-numeric: tabular-nums;
	}

	.note {
		margin: 0;
		padding: 8px 10px;
		font-size: 11px;
		color: var(--dim);
	}

	.note.warn {
		color: var(--warn);
	}

	.ghost {
		position: fixed;
		z-index: 20;
		pointer-events: none;
		padding: 2px 7px;
		font-size: 10px;
		color: var(--text);
		background: var(--panel-2);
		border: 1px solid var(--accent);
		border-radius: 0;
	}

	.filter-footer {
		padding: 4px 8px;
		border-top: 1px solid var(--line);
		flex: none;
	}

	.toggle-hidden {
		background: none;
		border: none;
		color: var(--dim);
		font-size: 10px;
		cursor: pointer;
		padding: 2px 4px;
	}

	.toggle-hidden:hover {
		color: var(--text);
	}
</style>
