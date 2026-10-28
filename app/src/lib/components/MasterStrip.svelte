<script lang="ts">
	/**
	 * The Master bus channel strip on the far right of the Mixer view:
	 * master volume fader, master stereo meter, and master effect chain chips.
	 * Selecting this strip opens the Master devices in DetailPane.
	 */

	import { session } from '$lib/session.svelte';
	import { MASTER_TRACK } from '$lib/wire';

	interface Props {
		selected?: boolean;
		onselect?: () => void;
	}

	let { selected = false, onselect }: Props = $props();

	let draggedVolume = $state<number | null>(null);

	const snapshot = $derived(session.snapshot);
	const masterVolume = $derived(draggedVolume ?? snapshot?.masterVolume ?? 0.85);
	const masterLevel = $derived(Math.round(Math.min(1, Math.max(0, snapshot?.masterLevel ?? 0)) * 100));
	const masterEffects = $derived(snapshot?.masterEffects ?? []);

	const volumeDb = $derived.by(() => {
		if (masterVolume <= 0.001) return '-inf';
		const db = Math.round(20 * Math.log10(masterVolume / 0.85) * 10) / 10;
		return db > 0 ? `+${db}` : `${db}`;
	});

	function onVolumeInput(event: Event): void {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) return;
		const val = Number(input.value);
		draggedVolume = val;
		session.dispatch({ setMasterVolume: { volume: val } });
	}

	function resetVolume(): void {
		draggedVolume = 0.85;
		session.dispatch({ setMasterVolume: { volume: 0.85 } });
	}

	function selectMaster(): void {
		if (onselect) onselect();
		else session.selectTrack(MASTER_TRACK);
	}

	function toggleEffect(effIdx: number, currentEnabled: boolean): void {
		session.dispatch({
			setEffectEnabled: {
				track: MASTER_TRACK,
				index: effIdx,
				enabled: !currentEnabled
			}
		});
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="master-strip"
	class:selected
	role="group"
	aria-label="Master mixer channel"
	onclick={selectMaster}
>
	<div class="color-tag"></div>

	<header class="header">
		<span class="eyebrow">BUS</span>
		<span class="name">MASTER</span>
		<span class="badge">STEREO OUT</span>
	</header>

	<div class="devices">
		<div class="devices-label">MASTER FX</div>
		<div class="device-chips">
			{#each masterEffects as eff, effIdx (eff.id)}
				<button
					type="button"
					class="chip"
					class:bypassed={!eff.enabled}
					title="{eff.name} ({eff.enabled ? 'Enabled' : 'Bypassed'})"
					onclick={(e) => {
						e.stopPropagation();
						toggleEffect(effIdx, eff.enabled);
					}}
				>
					<span class="led" class:on={eff.enabled}></span>
					<span class="chip-name">{eff.name}</span>
				</button>
			{/each}
			{#if masterEffects.length === 0}
				<span class="no-inserts">Clean bus</span>
			{/if}
		</div>
	</div>

	<div class="fader-meter-area">
		<div class="meter-col">
			<div class="v-meter">
				<div
					class="v-meter-fill"
					class:warm={masterLevel >= 80 && masterLevel < 92}
					class:hot={masterLevel >= 92}
					style="height: {masterLevel}%"
				></div>
				<div class="meter-scale">
					<span>0</span>
					<span>-6</span>
					<span>-12</span>
					<span>-24</span>
					<span>-48</span>
				</div>
			</div>
		</div>

		<div class="fader-col">
			<input
				class="v-fader"
				type="range"
				min="0"
				max="1"
				step="0.01"
				value={masterVolume}
				aria-label="Master Volume"
				ondblclick={resetVolume}
				oninput={onVolumeInput}
			/>
		</div>
	</div>

	<button
		type="button"
		class="db-readout"
		aria-label="Reset volume to default"
		ondblclick={resetVolume}
	>
		{volumeDb} dB
	</button>

	<footer class="footer">
		<div class="tag">MAIN</div>
	</footer>
</div>

<style>
	.master-strip {
		display: flex;
		flex-direction: column;
		width: 110px;
		flex: none;
		background: #0d0d0d;
		border-left: 2px solid var(--border);
		box-sizing: border-box;
		position: relative;
		user-select: none;
		cursor: pointer;
		transition: background 0.1s ease;
	}

	.master-strip:hover {
		background: #141414;
	}

	.master-strip.selected {
		border-top: 2px solid var(--green);
		background: color-mix(in srgb, #0d0d0d 90%, var(--green) 10%);
	}

	.color-tag {
		height: 3px;
		width: 100%;
		background: var(--text-subtle);
		flex: none;
	}

	.header {
		padding: 6px 8px 4px 8px;
		display: flex;
		flex-direction: column;
		gap: 2px;
		border-bottom: 1px solid var(--border);
	}

	.eyebrow {
		font-size: 9px;
		color: var(--dim);
		letter-spacing: 0.05em;
	}

	.name {
		font-size: 13px;
		color: var(--text);
		letter-spacing: 0.05em;
	}

	.badge {
		font-size: 8px;
		padding: 1px 4px;
		border-radius: 0;
		align-self: flex-start;
		background: var(--border);
		color: var(--silver);
	}

	.devices {
		padding: 6px 8px;
		height: 84px;
		border-bottom: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		gap: 4px;
		overflow-y: auto;
	}

	.devices-label {
		font-size: 9px;
		color: var(--dim);
		letter-spacing: 0.05em;
	}

	.device-chips {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.chip {
		display: flex;
		align-items: center;
		gap: 4px;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 0;
		padding: 2px 4px;
		font-size: 10px;
		color: var(--silver);
		cursor: pointer;
		text-align: left;
	}

	.chip.bypassed {
		opacity: 0.5;
	}

	.chip-name {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		flex: 1;
	}

	.led {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--dim);
		flex: none;
	}

	.led.on {
		background: var(--green);
		box-shadow: 0 0 4px var(--green);
	}

	.no-inserts {
		font-size: 10px;
		color: var(--dim);
		font-style: italic;
		padding: 4px 0;
	}

	.fader-meter-area {
		flex: 1;
		min-height: 196px;
		display: flex;
		padding: 10px 8px;
		gap: 8px;
		justify-content: center;
		align-items: center;
		position: relative;
	}

	.meter-col {
		width: 14px;
		height: 100%;
		display: flex;
		align-items: center;
		position: relative;
	}

	.v-meter {
		width: 8px;
		height: 100%;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 0;
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		justify-content: flex-end;
	}

	.v-meter-fill {
		width: 100%;
		border-radius: 0;
		background: var(--green);
	}

	.v-meter-fill.warm {
		background: linear-gradient(to top, var(--green) 0%, var(--green) 70%, #eab308 100%);
	}

	.v-meter-fill.hot {
		background: linear-gradient(to top, var(--green) 0%, var(--green) 60%, #eab308 75%, var(--alert) 100%);
	}

	.meter-scale {
		position: absolute;
		left: 10px;
		top: 0;
		bottom: 0;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		font-size: 7px;
		color: var(--dim);
		pointer-events: none;
	}

	.fader-col {
		flex: 1;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.v-fader {
		-webkit-appearance: slider-vertical;
		appearance: slider-vertical;
		width: 26px;
		height: 100%;
		background: transparent;
		cursor: pointer;
		accent-color: var(--green);
	}

	.v-fader::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 18px;
		height: 8px;
		border-radius: 0;
		background: var(--green);
	}

	.v-fader::-moz-range-thumb {
		width: 18px;
		height: 8px;
		border-radius: 0;
		border: none;
		background: var(--green);
	}

	.db-readout {
		font-size: 11px;
		color: var(--silver);
		text-align: center;
		padding: 5px 0;
		border-top: 1px solid var(--border);
		cursor: pointer;
	}

	.footer {
		display: flex;
		border-top: 1px solid var(--border);
		padding: 6px;
		justify-content: center;
	}

	.tag {
		font-size: 9px;
		color: var(--dim);
		letter-spacing: 0.1em;
	}
</style>
