<script lang="ts">
	/**
	 * The file browser: a real directory listing over `list_samples`, a breadcrumb
	 * with a way up, and the two routes a sample takes to a track, a pointer drag
	 * onto a lane and a double-click that lands it on the selected track.
	 */

	import { onDestroy, onMount } from 'svelte';

	import { listSamples, samplesRoot } from '$lib/api';
	import { freeStart, gridStep, snapBeats } from '$lib/geometry';
	import { Latest } from '$lib/queue';
	import { SampleDrag } from '$lib/sampleDrag.svelte';
	import { session } from '$lib/session.svelte';
	import type { SampleEntry } from '$lib/wire';
	import Breadcrumbs from './Breadcrumbs.svelte';
	import SamplePreview from './SamplePreview.svelte';

	const latest = new Latest();

	let dir = $state<string | null>(null);
	let root = $state<string | null>(null);
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
			const blocks = session.snapshot?.tracks[track]?.blocks ?? [];
			const resolvedBeats =
				startBeats ?? snapBeats(freeStart(blocks), gridStep(session.zoom));
			session.selectTrack(track);
			session.dispatch({ importSample: { track, path: entry.path, startBeats: resolvedBeats } });
		}
	});

	onMount(() => {
		void open(null);
	});

	onDestroy(() => drag.destroy());

	function reason(error: unknown): string {
		return error instanceof Error ? error.message : String(error);
	}

	function sizeLabel(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	async function open(target: string | null): Promise<void> {
		const ticket = latest.begin();
		loading = true;
		try {
			const listing = await listSamples(target);
			const resolved = target ?? (await samplesRoot().catch(() => null));
			if (!latest.isCurrent(ticket)) return;
			if (target === null) root = resolved;
			dir = resolved;
			entries = listing;
			failure = null;
		} catch (error) {
			if (!latest.isCurrent(ticket)) return;
			entries = [];
			failure = `${target ?? 'the samples root'} could not be read: ${reason(error)}`;
		} finally {
			if (latest.isCurrent(ticket)) loading = false;
		}
	}

	function activateOnEnter(event: KeyboardEvent, activate: () => void): void {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		activate();
	}

	function importToSelected(entry: SampleEntry): void {
		const snapshot = session.snapshot;
		let track = snapshot?.cursor.track ?? 0;
		const current = snapshot?.tracks[track];
		if (current && current.kind !== 'audio') {
			const audioIdx = snapshot?.tracks.findIndex((t) => t.kind === 'audio') ?? -1;
			if (audioIdx >= 0) {
				track = audioIdx;
				session.selectTrack(track);
			}
		}
		const blocks = snapshot?.tracks[track]?.blocks ?? [];
		session.dispatch({
			importSample: { track, path: entry.path, startBeats: snapBeats(freeStart(blocks), gridStep(session.zoom)) }
		});
	}
</script>

<div class="browser">
	<Breadcrumbs path={dir} {root} onOpen={(path) => open(path)} />

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
							title="Drag onto a track, or press Enter or double-click to import into the selected track"
							ondragstart={(event) => event.preventDefault()}
							onpointerdown={(event) => drag.begin(event, entry)}
							onclick={() => (selected = entry)}
							ondblclick={() => importToSelected(entry)}
							onkeydown={(event) => activateOnEnter(event, () => importToSelected(entry))}
						>
							<span class="name">{entry.name}</span>
							<span class="meta">{sizeLabel(entry.size)}</span>
						</button>
					{:else if entry.kind === 'project'}
						<button
							class="row project"
							class:picked={selected?.path === entry.path}
							title="Press Enter or double-click to load this project"
							onclick={() => (selected = entry)}
							ondblclick={() => session.dispatch({ loadProject: { path: entry.path } })}
							onkeydown={(event) =>
								activateOnEnter(event, () =>
									session.dispatch({ loadProject: { path: entry.path } })
								)}
						>
							<span class="name">{entry.name}</span>
							<span class="meta">project</span>
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
		padding: 4px 0;
		list-style: none;
	}

	.row {
		justify-content: flex-start;
		gap: 8px;
		width: 100%;
		height: 26px;
		padding: 0 12px;
		font-size: 12px;
		letter-spacing: 0.02em;
		text-transform: none;
		text-align: left;
		color: var(--text);
		background: transparent;
	}

	div.row {
		display: flex;
		align-items: center;
	}

	.row:hover:not(:disabled) {
		background: var(--surface-hover);
		filter: none;
	}

	.row.wav {
		cursor: grab;
		user-select: none;
		touch-action: none;
	}

	.row.picked,
	.row.picked:hover:not(:disabled) {
		background: var(--green-soft);
		color: var(--green);
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
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--text-subtle);
		font-variant-numeric: tabular-nums;
	}

	.note {
		margin: 0;
		padding: 12px;
		font-size: 11px;
		color: var(--text-subtle);
	}

	.note.warn {
		color: var(--alert);
	}

	.ghost {
		position: fixed;
		z-index: 20;
		pointer-events: none;
		padding: 4px 8px;
		font-size: 11px;
		color: var(--on-green);
		background: var(--green);
		box-shadow: var(--shadow);
	}

	.filter-footer {
		flex: none;
		padding: 6px 12px;
		border-top: 1px solid var(--border-subtle);
	}

	.toggle-hidden {
		height: 20px;
		padding: 0;
		font-size: 10px;
		letter-spacing: 0.14em;
		color: var(--text-subtle);
		background: transparent;
	}

	.toggle-hidden:hover:not(:disabled) {
		color: var(--text);
		filter: none;
	}
</style>
