<script lang="ts">
	/**
	 * The instruments pane: lists built-in presets grouped by role. A double-click
	 * or Enter sets the preset on the selected track.
	 */

	import { groupPresets } from '$lib/presets';
	import { session } from '$lib/session.svelte';

	const snapshot = $derived(session.snapshot);
	const presets = $derived(snapshot?.presets ?? []);
	const groups = $derived(groupPresets(presets));
	const trackIndex = $derived(snapshot?.cursor.track ?? 0);
	const selected = $derived.by(() => {
		const current = session.snapshot;
		return current ? (current.tracks[current.cursor.track] ?? null) : null;
	});

	function pick(preset: string): void {
		session.dispatch({ setPreset: { track: trackIndex, preset } });
	}

	function onKeyDown(event: KeyboardEvent, preset: string): void {
		if (event.key === 'Enter') {
			event.preventDefault();
			pick(preset);
		}
	}
</script>

<div class="pane">
	<h2 class="eyebrow">Instruments</h2>
	<p class="note">Built-in instruments. Plugin hosting is not implemented.</p>

	{#if snapshot === null}
		<p class="note">connecting…</p>
	{:else}
		<div class="groups" role="list" aria-label="Instrument presets">
			{#each groups as group (group.role)}
				<div class="group-section" role="group" aria-label="{group.role} presets">
					<h3 class="group-heading">{group.role}</h3>
					<ul class="voices">
						{#each group.presets as preset (preset)}
							<li>
								<button
									type="button"
									class="voice"
									class:on={selected?.preset === preset}
									aria-pressed={selected?.preset === preset}
									onclick={() => pick(preset)}
									ondblclick={() => pick(preset)}
									onkeydown={(event) => onKeyDown(event, preset)}
								>
									<span class="name">{preset}</span>
									{#if selected?.preset === preset}
										<span class="badge">active</span>
									{/if}
								</button>
							</li>
						{/each}
					</ul>
				</div>
			{/each}
		</div>

		{#if selected !== null}
			<p class="current">
				{selected.name} plays <span class="strong">{selected.preset}</span> on a
				{selected.waveform.toLowerCase()} oscillator.
			</p>
		{/if}
	{/if}
</div>

<style>
	.pane {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		overflow-y: auto;
		padding: 12px 14px;
	}

	.eyebrow {
		margin: 0 0 10px;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.note {
		margin: 0 0 12px;
		font-size: 12px;
		line-height: 1.5;
		color: var(--dim);
	}

	.groups {
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin: 0;
		padding: 0;
	}

	.group-section {
		display: flex;
		flex-direction: column;
	}

	.group-heading {
		margin: 0 0 4px;
		font-size: 10px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.voices {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
		border-top: 1px solid var(--border-subtle);
	}

	.voices li {
		border-bottom: 1px solid var(--border-subtle);
	}

	.voice {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		height: 28px;
		padding: 0 10px;
		background: transparent;
		color: var(--muted);
		font-size: 12px;
		letter-spacing: 0.02em;
		text-transform: none;
		text-align: left;
	}

	.voice:hover:not(:disabled) {
		filter: none;
		background: var(--surface-hover);
		color: var(--text);
	}

	.voice.on {
		background: var(--green-soft);
		color: var(--green);
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.badge {
		flex: none;
		font-size: 9px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--green);
	}

	.current {
		margin: 12px 0 0;
		font-size: 12px;
		line-height: 1.5;
		color: var(--dim);
	}

	.strong {
		color: var(--text);
	}
</style>
