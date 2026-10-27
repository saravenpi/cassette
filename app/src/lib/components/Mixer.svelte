<script lang="ts">
	/**
	 * Ableton-style Mixer view: horizontal scroll container of vertical
	 * channel strips for every track, with the Master channel pinned on the right.
	 * Clicking any channel strip focuses that track and opens its devices in DetailPane.
	 */

	import { session } from '$lib/session.svelte';
	import ChannelStrip from './ChannelStrip.svelte';
	import MasterStrip from './MasterStrip.svelte';

	const snapshot = $derived(session.snapshot);
	const tracks = $derived(snapshot?.tracks ?? []);
	const cursorTrack = $derived(snapshot?.cursor.track ?? 0);

	let selectedMaster = $state(false);

	function onTrackSelect(index: number, event: MouseEvent): void {
		selectedMaster = false;
		session.selectTrack(index);
	}

	function onMasterSelect(): void {
		selectedMaster = true;
		session.selectTrack(999);
	}

	function addTrack(kind: 'midi' | 'audio'): void {
		session.addTrack(kind);
	}
</script>

<div class="mixer-view" role="region" aria-label="Mixer">
	<div class="channel-strips">
		{#each tracks as track, index (index)}
			<ChannelStrip
				{track}
				{index}
				selected={!selectedMaster && cursorTrack === index}
				onselect={(e) => onTrackSelect(index, e)}
			/>
		{/each}

		{#if tracks.length < 16}
			<div class="add-col">
				<button
					type="button"
					class="add-btn"
					title="Add Audio Track (Ctrl+T)"
					onclick={() => addTrack('audio')}
				>
					+ Audio
				</button>
				<button
					type="button"
					class="add-btn"
					title="Add MIDI Track (Ctrl+Shift+T)"
					onclick={() => addTrack('midi')}
				>
					+ MIDI
				</button>
			</div>
		{/if}
	</div>

	<MasterStrip
		selected={selectedMaster || cursorTrack === 999}
		onselect={onMasterSelect}
	/>
</div>

<style>
	.mixer-view {
		flex: 1;
		min-height: 0;
		display: flex;
		background: var(--bg);
		overflow: hidden;
		position: relative;
	}

	.channel-strips {
		flex: 1;
		min-width: 0;
		display: flex;
		overflow-x: auto;
		overflow-y: hidden;
	}

	.add-col {
		width: 80px;
		flex: none;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 12px 8px;
		border-right: 1px dashed var(--border-subtle);
		background: var(--bg);
		justify-content: flex-start;
	}

	.add-btn {
		height: 28px;
		background: var(--surface);
		border: 1px dashed var(--border);
		border-radius: 3px;
		color: var(--dim);
		font-size: 10px;
		cursor: pointer;
		transition: border-color 0.1s, color 0.1s, background 0.1s;
	}

	.add-btn:hover {
		border-color: var(--green);
		color: var(--silver);
		background: var(--surface-hover);
	}
</style>
