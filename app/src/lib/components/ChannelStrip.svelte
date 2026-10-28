<script lang="ts">
	/**
	 * An Ableton-style vertical channel strip for the Mixer view:
	 * track header, device chain chips, pan dial, vertical dB fader,
	 * vertical level meter, and mute/solo/arm buttons.
	 */

	import { meterClips, meterPercent } from '$lib/geometry';
	import { trackColor } from '$lib/palette';
	import { session } from '$lib/session.svelte';
	import type { TrackView } from '$lib/wire';

	interface Props {
		track: TrackView;
		index: number;
		selected?: boolean;
		onselect?: (event: MouseEvent) => void;
	}

	let { track, index, selected = false, onselect }: Props = $props();

	let draggedVolume = $state<number | null>(null);
	let draggedPan = $state<number | null>(null);
	let renaming = $state(false);
	let renameText = $state('');
	let renameInput = $state<HTMLInputElement | null>(null);

	const DEFAULT_VOLUME = 0.8;
	const DEFAULT_PAN = 0.0;

	const color = $derived(trackColor(track.colorIndex));
	const volume = $derived(draggedVolume ?? track.volume);
	const pan = $derived(draggedPan ?? track.pan);
	const level = $derived(Math.round(meterPercent(track.level)));
	const clipping = $derived(meterClips(track.level));
	const isArmed = $derived(session.armedAudioTrack === index);
	const isRecordingAudio = $derived(session.audioTake?.track === index);

	const isPrimary = $derived(selected && session.snapshot?.cursor.track === index);

	const volumeDb = $derived.by(() => {
		if (volume <= 0.001) return '-inf';
		const db = Math.round(20 * Math.log10(volume / DEFAULT_VOLUME) * 10) / 10;
		return db > 0 ? `+${db}` : `${db}`;
	});

	const panLabel = $derived.by(() => {
		const p = Math.round(pan * 50);
		if (p === 0) return 'C';
		return p < 0 ? `L${Math.abs(p)}` : `R${p}`;
	});

	$effect(() => {
		if (draggedVolume !== null && Math.abs(track.volume - draggedVolume) < 0.001) {
			draggedVolume = null;
		}
		if (draggedPan !== null && Math.abs(track.pan - draggedPan) < 0.001) {
			draggedPan = null;
		}
	});

	$effect(() => {
		if (renaming && renameInput) {
			renameInput.focus();
			renameInput.select();
		}
	});

	function mix(nextVolume: number, nextPan: number, muted: boolean, solo: boolean): void {
		session.queueMix(index, nextVolume, nextPan, muted, solo);
	}

	function releaseFaders(): void {
		draggedVolume = null;
		draggedPan = null;
	}

	function onVolumeInput(event: Event): void {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) return;
		const val = Number(input.value);
		draggedVolume = val;
		mix(val, pan, track.muted, track.solo);
	}

	function onPanInput(event: Event): void {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) return;
		const val = Number(input.value);
		draggedPan = val;
		mix(volume, val, track.muted, track.solo);
	}

	function resetFader(which: 'volume' | 'pan'): void {
		session.dispatch('checkpoint');
		if (which === 'volume') {
			draggedVolume = DEFAULT_VOLUME;
			mix(DEFAULT_VOLUME, pan, track.muted, track.solo);
		} else {
			draggedPan = DEFAULT_PAN;
			mix(volume, DEFAULT_PAN, track.muted, track.solo);
		}
	}

	function startRename(): void {
		renameText = track.name;
		renaming = true;
	}

	function commitRename(): void {
		if (!renaming) return;
		renaming = false;
		const trimmed = renameText.trim();
		if (trimmed && trimmed !== track.name) {
			session.renameTrack(index, trimmed);
		}
	}

	function onRenameKey(event: KeyboardEvent): void {
		event.stopPropagation();
		if (event.key === 'Enter') {
			event.preventDefault();
			commitRename();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			renaming = false;
		}
	}

	function toggleEffect(effIdx: number, currentEnabled: boolean): void {
		session.dispatch({
			setEffectEnabled: {
				track: index,
				index: effIdx,
				enabled: !currentEnabled
			}
		});
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="strip"
	class:selected
	class:primary={isPrimary}
	data-strip-index={index}
	role="group"
	aria-label="{track.name} mixer channel"
	onclick={(e) => onselect ? onselect(e) : session.selectTrack(index)}
>
	<div class="color-tag" style="background: {color}"></div>

	<header class="header">
		<span class="index">{index + 1}</span>
		{#if renaming}
			<input
				bind:this={renameInput}
				class="rename-input"
				bind:value={renameText}
				onkeydown={onRenameKey}
				onblur={commitRename}
			/>
		{:else}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<span class="name" ondblclick={startRename} title="{track.name} (double-click to rename)">
				{track.name}
			</span>
		{/if}
		<span class="kind-chip" class:midi={track.kind === 'midi'}>
			{track.kind === 'midi' ? 'MIDI' : 'AUDIO'}
		</span>
	</header>

	<div class="devices">
		<div class="devices-label">INSERTS</div>
		<div class="device-chips">
			{#each track.effects as eff, effIdx (eff.id)}
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
			{#if track.effects.length === 0}
				<span class="no-inserts">Empty</span>
			{/if}
		</div>
	</div>

	<div class="pan-section">
		<div class="label-row">
			<span class="label">PAN</span>
			<span class="val">{panLabel}</span>
		</div>
		<input
			class="pan-slider"
			type="range"
			min="-1"
			max="1"
			step="0.02"
			value={pan}
			aria-label="Pan of {track.name}"
			onclick={(e) => e.stopPropagation()}
			onpointerdown={(event) => {
				event.stopPropagation();
				if (event.detail === 2) {
					resetFader('pan');
				} else {
					session.dispatch('checkpoint');
				}
			}}
			ondblclick={() => resetFader('pan')}
			oninput={onPanInput}
			onchange={releaseFaders}
			onblur={releaseFaders}
		/>
	</div>

	<div class="fader-meter-area">
		<div class="meter-col">
			<div class="v-meter">
				<div
					class="v-meter-fill"
					class:warm={track.level >= 0.5 && !clipping}
					class:hot={clipping}
					style="height: {level}%"
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
			<div class="v-fader-track" aria-hidden="true">
				<div class="v-fader-fill" style="height: {Math.round(volume * 100)}%"></div>
				<div
					class="v-fader-thumb"
					style="bottom: calc({Math.round(volume * 1000) / 10}% - 4px)"
				></div>
			</div>
			<input
				class="v-fader"
				type="range"
				min="0"
				max="1"
				step="0.01"
				value={volume}
				aria-label="Volume of {track.name}"
				onclick={(e) => e.stopPropagation()}
				onpointerdown={(event) => {
					event.stopPropagation();
					if (event.detail === 2) {
						resetFader('volume');
					} else {
						session.dispatch('checkpoint');
					}
				}}
				ondblclick={() => resetFader('volume')}
				oninput={onVolumeInput}
				onchange={releaseFaders}
				onblur={releaseFaders}
			/>
		</div>
	</div>

	<button
		type="button"
		class="db-readout"
		aria-label="Reset volume to default"
		onclick={(e) => e.stopPropagation()}
		ondblclick={() => resetFader('volume')}
	>
		{volumeDb} dB
	</button>

	<footer class="footer">
		{#if track.kind === 'audio'}
			<button
				type="button"
				class="btn arm"
				class:on={isArmed || isRecordingAudio}
				aria-pressed={isArmed || isRecordingAudio}
				title={isArmed
					? 'Disarm audio recording'
					: 'Arm audio track for recording (F9 to record)'}
				onclick={(e) => {
					e.stopPropagation();
					session.toggleArmAudioTrack(index);
				}}
			>
				●
			</button>
		{/if}
		<button
			type="button"
			class="btn mute"
			class:on={track.muted}
			aria-pressed={track.muted}
			title="Mute {track.name}"
			onclick={(e) => {
				e.stopPropagation();
				session.dispatch('checkpoint');
				mix(volume, pan, !track.muted, track.solo);
			}}
		>
			M
		</button>
		<button
			type="button"
			class="btn solo"
			class:on={track.solo}
			aria-pressed={track.solo}
			title="Solo {track.name}"
			onclick={(e) => {
				e.stopPropagation();
				session.dispatch('checkpoint');
				mix(volume, pan, track.muted, !track.solo);
			}}
		>
			S
		</button>
	</footer>
</div>

<style>
	.strip {
		display: flex;
		flex-direction: column;
		width: 104px;
		flex: none;
		background: var(--surface);
		border-right: 1px solid var(--border-subtle);
		border-left: 1px solid transparent;
		box-sizing: border-box;
		position: relative;
		user-select: none;
		transition: background 0.1s ease;
	}

	.strip:hover {
		background: color-mix(in srgb, var(--surface) 95%, var(--text) 5%);
	}

	.strip.selected {
		background: color-mix(in srgb, var(--surface) 88%, var(--green) 12%);
		border-left-color: var(--green);
		border-right-color: var(--green);
	}

	.strip.primary {
		border-top: 2px solid var(--green);
	}

	.color-tag {
		height: 3px;
		width: 100%;
		flex: none;
	}

	.header {
		padding: 6px 6px 4px 6px;
		display: flex;
		flex-direction: column;
		gap: 2px;
		border-bottom: 1px solid var(--border-subtle);
	}

	.index {
		font-size: 10px;
		color: var(--dim);
	}

	.name {
		font-size: 12px;
		color: var(--text);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		cursor: default;
	}

	.rename-input {
		font-size: 12px;
		background: var(--bg);
		color: var(--text);
		border: 1px solid var(--green);
		padding: 2px 4px;
		outline: none;
		width: 100%;
		box-sizing: border-box;
	}

	.kind-chip {
		font-size: 9px;
		padding: 1px 4px;
		border-radius: 0;
		align-self: flex-start;
		background: var(--border-subtle);
		color: var(--muted);
		text-transform: uppercase;
	}

	.kind-chip.midi {
		color: var(--olive);
	}

	.devices {
		padding: 6px 6px;
		height: 84px;
		border-bottom: 1px solid var(--border-subtle);
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
		border: 1px solid var(--border-subtle);
		border-radius: 0;
		padding: 2px 4px;
		font-size: 10px;
		color: var(--silver);
		cursor: pointer;
		text-align: left;
		transition: opacity 0.1s;
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

	.pan-section {
		padding: 6px 6px;
		border-bottom: 1px solid var(--border-subtle);
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.label-row {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		color: var(--dim);
	}

	.pan-slider {
		-webkit-appearance: none;
		appearance: none;
		height: 4px;
		background: var(--bg);
		border-radius: 0;
		outline: none;
		cursor: pointer;
		width: 100%;
		accent-color: var(--green);
	}

	.pan-slider::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 10px;
		height: 10px;
		border-radius: 0 !important;
		background: var(--green);
		cursor: pointer;
	}

	.pan-slider:hover::-webkit-slider-thumb,
	.pan-slider:active::-webkit-slider-thumb {
		background: var(--green);
		border-radius: 0 !important;
	}

	.pan-slider::-moz-range-thumb {
		width: 10px;
		height: 10px;
		border-radius: 0 !important;
		border: none;
		background: var(--green);
		cursor: pointer;
	}

	.pan-slider:hover::-moz-range-thumb,
	.pan-slider:active::-moz-range-thumb {
		background: var(--green);
		border-radius: 0 !important;
	}

	.fader-meter-area {
		flex: 1;
		min-height: 160px;
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
		width: 6px;
		height: 100%;
		background: var(--bg);
		border: 1px solid var(--border-subtle);
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
		left: 8px;
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
		position: relative;
		flex: 1;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.v-fader-track {
		position: relative;
		width: 4px;
		height: calc(100% - 8px);
		background: var(--bg);
		border: 1px solid var(--border-subtle);
		border-radius: 0;
		pointer-events: none;
		box-sizing: border-box;
	}

	.v-fader-fill {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 0;
		background: var(--green-soft);
	}

	.v-fader-thumb {
		position: absolute;
		left: 50%;
		transform: translateX(-50%);
		width: 20px;
		height: 8px;
		background: var(--green);
		border-radius: 0;
	}

	.v-fader {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		margin: 0;
		opacity: 0;
		-webkit-appearance: slider-vertical;
		appearance: slider-vertical;
		writing-mode: vertical-lr;
		direction: rtl;
		cursor: pointer;
	}

	.db-readout {
		font-size: 10px;
		color: var(--muted);
		text-align: center;
		padding: 4px 0;
		border-top: 1px solid var(--border-subtle);
		cursor: pointer;
	}

	.footer {
		display: flex;
		border-top: 1px solid var(--border-subtle);
		padding: 4px;
		gap: 4px;
	}

	.btn {
		flex: 1;
		height: 24px;
		background: var(--bg);
		border: 1px solid var(--border-subtle);
		border-radius: 0;
		color: var(--muted);
		font-size: 11px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background 0.1s, color 0.1s;
	}

	.btn.mute.on {
		background: #ca8a04;
		color: #000000;
		border-color: #ca8a04;
	}

	.btn.solo.on {
		background: var(--blue);
		color: #000000;
		border-color: var(--blue);
	}

	.btn.arm.on {
		background: var(--alert);
		color: #000000;
		border-color: var(--alert);
	}
</style>
