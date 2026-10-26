<script lang="ts">
	/**
	 * The status bar: the engine's last word, a failure when one is standing, the
	 * sample rate and the keys worth keeping in mind.
	 */

	import { session } from '$lib/session.svelte';

	const snapshot = $derived(session.snapshot);
	const error = $derived(session.error);
</script>

<footer>
	<span class="status" role="status" class:renaming={snapshot?.renaming}>
		{snapshot?.status ?? 'connecting…'}
	</span>

	<span class="error" role="alert">
		{#if error !== null}{error}{/if}
	</span>

	<span class="keys">
		<kbd>space</kbd> play
		<kbd>L</kbd> loop
		<kbd>N</kbd> add
		<kbd>del</kbd> delete
		<kbd>R</kbd> rename
		<kbd>C</kbd> kind
		<kbd>M</kbd> mute
		<kbd>O</kbd> solo
		<kbd>&minus;</kbd><kbd>=</kbd> vol
		<kbd>shift</kbd><kbd>+</kbd> zoom
		<kbd>home</kbd> start
	</span>

	<span class="rate">{snapshot?.sampleRate ?? 0} Hz</span>
</footer>

<style>
	footer {
		display: flex;
		align-items: center;
		flex: none;
		gap: 16px;
		height: 28px;
		padding: 0 16px;
		background: var(--bg);
		border-top: 1px solid var(--border-subtle);
		font-size: 11px;
		color: var(--text-subtle);
	}

	.status {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.status.renaming {
		color: var(--olive);
	}

	.error {
		color: var(--alert);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.keys {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: 6px;
		overflow: hidden;
		white-space: nowrap;
	}

	.rate {
		flex: none;
		font-variant-numeric: tabular-nums;
	}
</style>
