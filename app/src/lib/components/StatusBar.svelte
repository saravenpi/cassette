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
	<span class="status" class:renaming={snapshot?.renaming}>{snapshot?.status ?? 'connecting…'}</span>

	{#if error !== null}
		<span class="error">{error}</span>
	{/if}

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
		gap: 14px;
		padding: 4px 12px;
		background: var(--panel);
		border-top: 1px solid var(--line);
		font-size: 11px;
		min-height: 24px;
	}

	.status {
		color: var(--dim);
	}

	.status.renaming {
		color: var(--head);
	}

	.error {
		color: var(--warn);
		font-family: ui-monospace, monospace;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.keys {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--dim);
		overflow: hidden;
		white-space: nowrap;
	}

	.rate {
		color: var(--dim);
		font-variant-numeric: tabular-nums;
	}
</style>
